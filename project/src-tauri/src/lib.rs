use serde::Serialize;
use std::fs::{OpenOptions, File};
use std::io::{Write, BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::ERROR_SUCCESS;

const LOG_PATH: &str = "D:\\code\\network-stats.log";
const MAX_HEAD_LINES: usize = 200;
const MAX_TAIL_LINES: usize = 200;

// 日志滚动：当文件超过限制时，保留头部和尾部
fn rotate_log_if_needed() {
    let file = match File::open(LOG_PATH) {
        Ok(f) => f,
        Err(_) => return,
    };

    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();

    // 如果行数超过限制，进行滚动
    if lines.len() > MAX_HEAD_LINES + MAX_TAIL_LINES {
        let head: Vec<&String> = lines.iter().take(MAX_HEAD_LINES).collect();
        let tail: Vec<&String> = lines.iter().skip(lines.len() - MAX_TAIL_LINES).collect();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut output = match File::create(LOG_PATH) {
            Ok(f) => f,
            Err(_) => return,
        };

        // 写入头部
        for line in &head {
            let _ = writeln!(output, "{}", line);
        }

        // 写入分隔符
        let _ = writeln!(output, "");
        let _ = writeln!(output, "--- === 日志滚动于 {}，已省略 {} 行 === ---",
            now, lines.len() - MAX_HEAD_LINES - MAX_TAIL_LINES);
        let _ = writeln!(output, "");

        // 写入尾部
        for line in &tail {
            let _ = writeln!(output, "{}", line);
        }
    }
}

macro_rules! log_msg {
    ($($arg:tt)*) => {
        {
            let msg = format!($($arg)*);
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .write(true)
                .open(LOG_PATH) {
                let _ = writeln!(file, "{}", msg);
            }
            // 检查是否需要滚动日志（每100条日志检查一次，避免频繁IO）
            // 使用静态计数器
            static mut LOG_COUNTER: u32 = 0;
            unsafe {
                LOG_COUNTER += 1;
                if LOG_COUNTER % 100 == 0 {
                    rotate_log_if_needed();
                }
            }
        }
    };
}

#[derive(Serialize, Clone, Default)]
struct NetworkStats {
    latency: u32,
    download_speed: f64,
    upload_speed: f64,
    packet_loss: f64,
    status: String,
}

#[derive(Default)]
struct NetworkState {
    last_bytes_received: u64,
    last_bytes_sent: u64,
    last_bytes_update: Option<Instant>,
    current_stats: NetworkStats,
    cached_received: u64,
    cached_sent: u64,
    last_latency_update: Option<Instant>,
}

static mut BG_THREAD_HANDLE: Option<thread::JoinHandle<()>> = None;

// IP and Weather structures
#[derive(Serialize, Clone)]
struct IPInfo {
    ip: String,
    city: String,
    country: String,
    timezone: String,  // 新增：IP所在地的时区
}

#[derive(Serialize, Clone)]
struct WeatherInfo {
    temp: String,
    desc: String,
    location: String,
    country: String,
    local_time: String,
    icon: String,
}

// Helper: extract IP from text
fn extract_ip(text: &str) -> Option<String> {
    let _trimmed = text.trim();

    // Use regex-like pattern to find IP: xxx.xxx.xxx.xxx
    // Look for pattern where we have digits.digits.digits.digits
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Try to find an IP starting at position i
        let mut octet_start = i;
        let mut octets = Vec::new();
        let mut valid = true;

        for octet_num in 0..4 {
            // Find start of octet (digit)
            while octet_start < chars.len() && !chars[octet_start].is_ascii_digit() {
                octet_start += 1;
            }

            if octet_start >= chars.len() {
                valid = false;
                break;
            }

            // Find end of octet
            let mut octet_end = octet_start;
            while octet_end < chars.len() && chars[octet_end].is_ascii_digit() {
                octet_end += 1;
            }

            let octet_str: String = chars[octet_start..octet_end].iter().collect();
            let octet_val: u32 = octet_str.parse().unwrap_or(256);

            if octet_val > 255 {
                valid = false;
                break;
            }

            octets.push(octet_str);

            // Check for dot between octets (except after last octet)
            if octet_num < 3 {
                if octet_end >= chars.len() || chars[octet_end] != '.' {
                    valid = false;
                    break;
                }
                octet_start = octet_end + 1;
            } else {
                // After 4th octet, should not be followed by digit or dot
                if octet_end < chars.len() && (chars[octet_end].is_ascii_digit() || chars[octet_end] == '.') {
                    // Check if next char could extend the IP (more digits or octets)
                    if octet_end < chars.len() && chars[octet_end].is_ascii_digit() {
                        valid = false;
                    }
                }
            }
        }

        if valid && octets.len() == 4 {
            return Some(format!("{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3]));
        }

        i += 1;
    }

    None
}

// Helper: extract city from Chinese response
fn extract_city(text: &str) -> String {
    let cities = [
        ("上海", "Shanghai"),
        ("北京", "Beijing"),
        ("广州", "Guangzhou"),
        ("深圳", "Shenzhen"),
        ("天津", "Tianjin"),
        ("杭州", "Hangzhou"),
        ("成都", "Chengdu"),
        ("重庆", "Chongqing"),
        ("武汉", "Wuhan"),
        ("西安", "Xian"),
    ];

    for (chinese, english) in cities.iter() {
        if text.contains(chinese) {
            return english.to_string();
        }
    }

    "本地".to_string()
}

// Get network bytes using Windows API
#[cfg(target_os = "windows")]
fn get_network_bytes_api() -> Option<(u64, u64)> {
    use windows::Win32::NetworkManagement::IpHelper::{
        GetIfTable2, FreeMibTable, MIB_IF_TABLE2,
    };

    unsafe {
        let mut if_table_ptr: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        let ret = GetIfTable2(&mut if_table_ptr);

        if ret != ERROR_SUCCESS || if_table_ptr.is_null() {
            return None;
        }

        let table = &*if_table_ptr;
        let mut total_received = 0u64;
        let mut total_sent = 0u64;
        let mut active_count = 0u32;

        const IF_OPER_STATUS_OPERATIONAL: i32 = 1;
        const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;

        let rows_ptr = table.Table.as_ptr();
        for i in 0..table.NumEntries as isize {
            let row = &*rows_ptr.offset(i);

            if row.OperStatus.0 != IF_OPER_STATUS_OPERATIONAL {
                continue;
            }

            let if_type = row.Type;
            if if_type == IF_TYPE_SOFTWARE_LOOPBACK {
                continue;
            }

            total_received += row.InOctets;
            total_sent += row.OutOctets;
            active_count += 1;
        }

        FreeMibTable(if_table_ptr as _);

        if active_count == 0 {
            None
        } else {
            log_msg!("API: {} active interfaces", active_count);
            Some((total_received, total_sent))
        }
    }
}

// PowerShell fallback for network bytes
#[cfg(target_os = "windows")]
fn get_network_bytes_ps() -> Option<(u64, u64)> {
    let script = r#"
        Get-NetAdapter | Where-Object { $_.Status -eq 'Up' } | ForEach-Object {
            $stats = Get-NetAdapterStatistics -Name $_.Name -ErrorAction SilentlyContinue
            if ($stats) {
                Write-Output "$($stats.ReceivedBytes),$($stats.SentBytes)"
            }
        }
    "#;

    let output = std::process::Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(0x08000000)
        .output();

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let trimmed = stdout.trim();

            if let Some(pos) = trimmed.find(',') {
                let received = trimmed[..pos].trim().parse::<u64>().unwrap_or(0);
                let sent = trimmed[pos + 1..].trim().parse::<u64>().unwrap_or(0);
                log_msg!("PS: recv={}, sent={}", received, sent);
                Some((received, sent))
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

#[cfg(target_os = "windows")]
fn get_network_bytes() -> Option<(u64, u64)> {
    get_network_bytes_api().or_else(get_network_bytes_ps)
}

#[cfg(not(target_os = "windows"))]
fn get_network_bytes() -> Option<(u64, u64)> {
    Some((0, 0))
}

// Ping gateway
#[cfg(target_os = "windows")]
fn ping_gateway_internal() -> (u32, f64) {
    use std::process::Command;

    log_msg!("Pinging gateway...");

    let gateway_output = Command::new("powershell")
        .args([
            "-WindowStyle", "Hidden",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-NetRoute -DestinationPrefix '0.0.0.0/0' | Select-Object -First 1).NextHop",
        ])
        .creation_flags(0x08000000)
        .output();

    let target_ip = if let Ok(result) = gateway_output {
        let ip = String::from_utf8_lossy(&result.stdout).trim().to_string();
        if !ip.is_empty() && ip.contains('.') {
            log_msg!("Gateway: {}", ip);
            ip
        } else {
            log_msg!("No valid gateway, using 8.8.8.8");
            "8.8.8.8".to_string()
        }
    } else {
        log_msg!("Failed to get gateway, using 8.8.8.8");
        "8.8.8.8".to_string()
    };

    let output = Command::new("ping")
        .args(["-n", "1", "-w", "2000", &target_ip])
        .creation_flags(0x08000000)
        .output();

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            log_msg!("Ping output: {}", stdout.trim());

            // Check for packet loss
            if stdout.contains("100% loss") || stdout.contains("timed out") ||
               stdout.contains("unreachable") || stdout.contains("General failure") {
                log_msg!("Ping: packet loss detected");
                return (0, 100.0);
            }

            // Parse latency
            for line in stdout.lines() {
                if line.contains("ms") || line.contains("MS") {
                    let ms_pos = line.find("ms").or_else(|| line.find("MS")).unwrap_or(0);
                    if ms_pos > 2 {
                        let before_ms = &line[..ms_pos];
                        if let Some(last_space) = before_ms.rfind(' ') {
                            let num_str = &before_ms[last_space + 1..];
                            if let Ok(latency) = num_str.trim().parse::<f64>() {
                                log_msg!("Latency: {}ms", latency);
                                return (latency as u32, 0.0);
                            }
                        } else if let Some(last_eq) = before_ms.rfind('=') {
                            let num_str = &before_ms[last_eq + 1..];
                            if let Ok(latency) = num_str.trim().parse::<f64>() {
                                log_msg!("Latency: {}ms", latency);
                                return (latency as u32, 0.0);
                            }
                        }
                    }
                }
            }

            // If bytes and TTL present but no time, it's <1ms
            if stdout.contains("bytes=") && stdout.contains("TTL=") {
                log_msg!("Latency: <1ms");
                return (1, 0.0);
            }

            if stdout.contains("TTL=") {
                log_msg!("Ping succeeded but no time, using default");
                return (5, 0.0);
            }

            log_msg!("Ping parsing failed");
            (0, 100.0)
        }
        Err(e) => {
            log_msg!("Ping error: {}", e);
            (0, 100.0)
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn ping_gateway_internal() -> (u32, f64) {
    (30, 0.0)
}

#[cfg(target_os = "windows")]
fn ping_gateway() -> (u32, f64) {
    ping_gateway_internal()
}

// Background updater
fn background_updater(state: Arc<Mutex<NetworkState>>) {
    log_msg!("Background updater thread started");

    loop {
        thread::sleep(Duration::from_secs(1));

        let mut state_guard = match state.lock() {
            Ok(g) => g,
            Err(_) => {
                log_msg!("Failed to lock state in background thread");
                continue;
            }
        };

        let now = Instant::now();

        // Get current network bytes
        let (current_received, current_sent) = get_network_bytes()
            .unwrap_or((state_guard.cached_received, state_guard.cached_sent));

        // Calculate speeds
        let (download_speed, upload_speed) = if let Some(last_time) = state_guard.last_bytes_update {
            let elapsed = now.duration_since(last_time).as_secs_f64();

            if elapsed >= 0.5 {
                let delta_received = if current_received >= state_guard.last_bytes_received {
                    current_received - state_guard.last_bytes_received
                } else {
                    current_received
                };

                let delta_sent = if current_sent >= state_guard.last_bytes_sent {
                    current_sent - state_guard.last_bytes_sent
                } else {
                    current_sent
                };

                let dl_speed = if elapsed > 0.0 {
                    (delta_received as f64 / elapsed) / 1024.0
                } else {
                    0.0
                };

                let ul_speed = if elapsed > 0.0 {
                    (delta_sent as f64 / elapsed) / 1024.0
                } else {
                    0.0
                };

                state_guard.last_bytes_received = current_received;
                state_guard.last_bytes_sent = current_sent;
                state_guard.last_bytes_update = Some(now);

                (dl_speed.min(1024000.0), ul_speed.min(1024000.0))
            } else {
                (state_guard.current_stats.download_speed, state_guard.current_stats.upload_speed)
            }
        } else {
            // First run
            state_guard.last_bytes_received = current_received;
            state_guard.last_bytes_sent = current_sent;
            state_guard.last_bytes_update = Some(now);
            (0.0, 0.0)
        };

        state_guard.cached_received = current_received;
        state_guard.cached_sent = current_sent;

        // Update latency/packet loss every 10 seconds
        let seconds_since_last_ping = state_guard.last_latency_update
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(10);

        let (latency, packet_loss) = if seconds_since_last_ping >= 10 {
            let (lat, pl) = ping_gateway();
            state_guard.last_latency_update = Some(now);
            (lat, pl)
        } else {
            (state_guard.current_stats.latency, state_guard.current_stats.packet_loss)
        };

        // Calculate status
        let status = if latency == 0 && seconds_since_last_ping < 2 {
            "检测中...".to_string()
        } else if latency > 100 || packet_loss > 5.0 {
            "较差".to_string()
        } else if latency > 50 || packet_loss > 2.0 {
            "一般".to_string()
        } else {
            "良好".to_string()
        };

        // Update cached stats
        state_guard.current_stats = NetworkStats {
            latency,
            download_speed,
            upload_speed,
            packet_loss,
            status: status.clone(),
        };

        log_msg!("BG: DL={:.2} UL={:.2} Lat={}ms PL={:.1} {}",
            download_speed, upload_speed, latency, packet_loss, status);
    }
}

// Tauri commands
#[tauri::command]
fn get_network_stats(
    state: tauri::State<Arc<Mutex<NetworkState>>>,
) -> NetworkStats {
    let state_guard = state.lock().unwrap();
    state_guard.current_stats.clone()
}

#[tauri::command]
fn test_command() -> String {
    log_msg!("Test command called!");
    "Test OK".to_string()
}

#[tauri::command]
async fn get_public_ip() -> Result<IPInfo, String> {
    log_msg!("=== Fetching public IP ===");

    // 优先使用能返回地理位置的 JSON API
    // ip-api.com 免费版无需 API key，但限制 45req/min
    let apis = [
        ("http://ip-api.com/json/", "json"),
        ("https://ipapi.co/json/", "json"),
        ("https://api.ipify.org?format=json", "json"),
        ("https://api.ipify.org", "plain"),
        ("https://ifconfig.me/ip", "plain"),
        ("http://myip.ipip.net", "chinese"),
    ];

    // 先尝试 JSON API 获取完整信息
    for (url, api_type) in apis.iter() {
        // 跳过非 JSON API，稍后再试
        if *api_type != "json" {
            continue;
        }

        log_msg!("Trying JSON API: {}", url);

        // 使用 format! 构建脚本，确保 URL 被正确插入
        let ps_script = format!(r#"
                try {{
                    $client = New-Object System.Net.WebClient
                    $client.Headers.Add("User-Agent", "Mozilla/5.0")
                    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
                    $result = $client.DownloadString("{}")
                    Write-Output $result
                }} catch {{
                    Write-Output "ERROR: $($_.Exception.Message)"
                }}
            "#, url);

        match std::process::Command::new("powershell")
            .args(["-WindowStyle", "Hidden", "-NoProfile", "-NonInteractive", "-Command", &ps_script])
            .creation_flags(0x08000000)
            .output() {
            Ok(result) => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                let stderr = String::from_utf8_lossy(&result.stderr);
                let combined = format!("{}\n{}", stdout, stderr);
                let trimmed = combined.trim();

                log_msg!("JSON API {} response: '{}'", url, trimmed);

                if trimmed.starts_with("ERROR:") || trimmed.is_empty() {
                    log_msg!("API {} failed, trying next", url);
                    continue;
                }

                // 尝试解析 JSON
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&trimmed) {
                    // 检查是否有错误
                    if let Some(_status) = data.get("status").and_then(|v| v.as_str()) {
                        if _status == "fail" {
                            log_msg!("API returned fail status for {}", url);
                            continue;
                        }
                    }

                    // 提取 IP
                    let ip = if let Some(v) = data.get("query").or_else(|| data.get("ip")) {
                        v.as_str().unwrap_or("").to_string()
                    } else {
                        continue;
                    };

                    if ip.is_empty() || !extract_ip(&ip).is_some() {
                        log_msg!("Invalid IP in JSON response from {}", url);
                        continue;
                    }

                    // 提取城市和地区 - 优先使用 region (州/省)
                    let city = data.get("regionName")
                        .or_else(|| data.get("region"))
                        .or_else(|| data.get("city"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string();

                    let country = data.get("country")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    // 提取时区信息
                    let timezone = data.get("timezone")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    log_msg!("Successfully got IP info: {} from {} - city: {}, country: {}, timezone: {}", ip, url, city, country, timezone);

                    return Ok(IPInfo {
                        ip,
                        city,
                        country,
                        timezone,
                    });
                } else {
                    log_msg!("Failed to parse JSON from {}", url);
                }
            }
            Err(e) => {
                log_msg!("PowerShell failed for {}: {}", url, e);
            }
        }
    }

    // JSON API 都失败了，尝试 plain API 只获取 IP
    log_msg!("JSON APIs failed, trying plain APIs for IP only");

    for (url, api_type) in apis.iter() {
        if *api_type == "json" {
            continue;
        }

        log_msg!("Trying plain API: {}", url);

        let ps_script = match api_type {
            &"plain" => format!(r#"
                try {{
                    $client = New-Object System.Net.WebClient
                    $client.Headers.Add("User-Agent", "Mozilla/5.0")
                    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
                    $ip = $client.DownloadString("{}")
                    $ip.Trim()
                }} catch {{
                    Write-Host "ERROR: $($_.Exception.Message)"
                }}
            "#, url),
            &"chinese" => format!(r#"
                try {{
                    $client = New-Object System.Net.WebClient
                    $client.Headers.Add("User-Agent", "Mozilla/5.0")
                    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
                    $result = $client.DownloadString("{}")
                    $result
                }} catch {{
                    Write-Host "ERROR: $($_.Exception.Message)"
                }}
            "#, url),
            _ => continue,
        };

        match std::process::Command::new("powershell")
            .args(["-WindowStyle", "Hidden", "-NoProfile", "-NonInteractive", "-Command", &ps_script])
            .creation_flags(0x08000000)
            .output() {
            Ok(result) => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                let stderr = String::from_utf8_lossy(&result.stderr);
                let combined = format!("{}\n{}", stdout, stderr);
                let trimmed = combined.trim();

                log_msg!("Plain API {} response: '{}'", url, trimmed);

                if trimmed.starts_with("ERROR:") || trimmed.is_empty() {
                    continue;
                }

                if let Some(ip) = extract_ip(&trimmed) {
                    log_msg!("Got IP from plain API: {} from {}", ip, url);

                    return Ok(IPInfo {
                        ip,
                        city: "Unknown".to_string(),  // plain API 无法获取城市
                        country: String::new(),
                        timezone: String::new(),  // plain API 无法获取时区
                    });
                }
            }
            Err(e) => {
                log_msg!("PowerShell failed for {}: {}", url, e);
            }
        }
    }

    Err("所有IP API都失败了".to_string())
}

#[tauri::command]
async fn get_weather(city: String, timezone: String) -> Result<WeatherInfo, String> {
    log_msg!("=== Fetching weather for: {} with timezone: {} ===", city, timezone);

    let url = format!("https://wttr.in/{}?format=j1&lang=zh",
        urlencoding::encode(&city));

    match reqwest::get(&url).await {
        Ok(response) => {
            let status = response.status();
            log_msg!("Weather API status: {}", status);

            if !status.is_success() {
                return Err(format!("API返回状态: {}", status));
            }

            let text = response.text().await.unwrap_or_default();

            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(data) => {
                    let current = data.get("current_condition")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| arr.first())
                        .ok_or("无法解析天气数据".to_string())?;

                    let area = data.get("nearest_area")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| arr.first())
                        .ok_or("无法解析地区数据".to_string())?;

                    let temp = current.get("temp_C")
                        .and_then(|v| v.as_str())
                        .unwrap_or("--");

                    let desc = current.get("weatherDesc")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|v| v.get("value"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("未知");

                    // 获取区域信息 - 优先使用 region（州/省）而不是 areaName（可能是小镇）
                    let region = area.get("region")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|v| v.get("value"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    let country = area.get("country")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|v| v.get("value"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    // 使用从 IP API 获取的 timezone 参数来计算当地时间
                    let local_time = if timezone.is_empty() {
                        get_china_time()
                    } else {
                        get_local_time_for_timezone(&timezone)
                    };

                    // 构建显示的 location 名称
                    // 对于美国：显示 "州名" 而不是具体城市
                    // 对于其他国家：使用请求的城市名
                    let location = if country == "United States of America" || country == "USA" {
                        // 美国显示州名
                        if !region.is_empty() {
                            region.to_string()
                        } else {
                            city.to_string()
                        }
                    } else if country == "China" || country == "中国" {
                        // 中国使用传入的城市名
                        city.to_string()
                    } else {
                        // 其他国家使用 region 或 city
                        if !region.is_empty() {
                            region.to_string()
                        } else {
                            city.to_string()
                        }
                    };

                    let icon = get_weather_icon(desc);
                    log_msg!("Weather: {}°C, {} in {} (region: {}, country: {}, timezone: {}, time: {})",
                        temp, desc, location, region, country, timezone, local_time);

                    Ok(WeatherInfo {
                        temp: format!("{}°C", temp),
                        desc: desc.to_string(),
                        location,
                        country: country.to_string(),
                        local_time,
                        icon,
                    })
                }
                Err(e) => {
                    log_msg!("JSON parse error: {}", e);
                    Err(format!("解析天气数据失败: {}", e))
                }
            }
        }
        Err(e) => {
            log_msg!("Weather API request failed: {}", e);
            Err(format!("天气 API 请求失败: {}", e))
        }
    }
}

// 根据时区字符串计算当地时间
fn get_local_time_for_timezone(timezone: &str) -> String {
    use std::time::SystemTime;

    if let Ok(now) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        let secs = now.as_secs();

        // 解析时区偏移（例如：UTC+08:00 或 UTC-05:00）
        let offset_seconds = if timezone.contains("UTC") || timezone.contains("GMT") {
            // ���取偏移数字
            let tz_upper = timezone.to_uppercase();
            let sign = if tz_upper.contains('+') { 1 } else if tz_upper.contains('-') { -1 } else { 0 };

            // 查找数字部分
            if let Some(start) = tz_upper.find(|c: char| c.is_ascii_digit() || c == '+' || c == '-') {
                let num_part = &tz_upper[start..];
                let parts: Vec<&str> = num_part.split(':').collect();
                if parts.len() >= 2 {
                    let hours: i64 = parts[0].chars().skip_while(|c| !c.is_ascii_digit()).take(2).collect::<String>().parse().unwrap_or(0);
                    let minutes: i64 = parts.get(1).unwrap_or(&"0").parse().unwrap_or(0);
                    sign * (hours * 3600 + minutes * 60)
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            // 尝试从常见时区名称映射
            let tz = timezone.to_uppercase();
            if tz.contains("SHANGHAI") || tz.contains("CHONGQING") || tz.contains("BEIJING") || tz.contains("CHINA") {
                8 * 3600  // UTC+8
            } else if tz.contains("TOKYO") || tz.contains("SEOUL") {
                9 * 3600  // UTC+9
            } else if tz.contains("NEW_YORK") || tz.contains("NEW YORK") || tz.contains("AMERICA/NEW_YORK") || tz.contains("EST") || tz.contains("EDT") {
                -5 * 3600  // UTC-5 (EST)
            } else if tz.contains("LOS_ANGELES") || tz.contains("PST") || tz.contains("PDT") {
                -8 * 3600  // UTC-8 (PST)
            } else if tz.contains("CHICAGO") || tz.contains("CST") || tz.contains("CDT") {
                -6 * 3600  // UTC-6 (CST)
            } else if tz.contains("DENVER") || tz.contains("MST") || tz.contains("MDT") {
                -7 * 3600  // UTC-7 (MST)
            } else if tz.contains("LONDON") || tz.contains("GMT") || tz.contains("BST") {
                0  // UTC+0
            } else if tz.contains("PARIS") || tz.contains("BERLIN") || tz.contains("ROME") {
                1 * 3600  // UTC+1
            } else if tz.contains("MOSCOW") {
                3 * 3600  // UTC+3
            } else if tz.contains("SYDNEY") || tz.contains("MELBOURNE") {
                11 * 3600  // UTC+11 (AEDT)
            } else if tz.contains("AUCKLAND") {
                13 * 3600  // UTC+13
            } else if tz.contains("DUBAI") {
                4 * 3600  // UTC+4
            } else {
                // 默认使用中国时间
                8 * 3600
            }
        };

        let total_secs = secs as i64 + offset_seconds;
        let days_offset = if total_secs < 0 { 86400 } else { 0 };
        let adjusted_secs = ((total_secs % 86400) + days_offset) as u64;
        let hours = (adjusted_secs % 86400) / 3600;
        let minutes = (adjusted_secs % 3600) / 60;
        format!("{:02}:{:02}", hours, minutes)
    } else {
        "--:--".to_string()
    }
}

fn get_china_time() -> String {
    get_local_time_for_timezone("Asia/Shanghai")
}

fn get_weather_icon(desc: &str) -> String {
    let d = desc.to_lowercase();
    if d.contains("sunny") || d.contains("clear") || d.contains("晴") {
        "☀️".to_string()
    } else if d.contains("cloud") || d.contains("overcast") || d.contains("阴") {
        "☁️".to_string()
    } else if d.contains("partly") || d.contains("cloudy") || d.contains("多云") {
        "⛅".to_string()
    } else if d.contains("rain") || d.contains("drizzle") || d.contains("雨") {
        "🌧️".to_string()
    } else if d.contains("snow") || d.contains("雪") {
        "❄️".to_string()
    } else if d.contains("thunder") || d.contains("storm") || d.contains("雷") {
        "⛈️".to_string()
    } else if d.contains("fog") || d.contains("雾") {
        "🌫️".to_string()
    } else {
        "🌤️".to_string()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    log_msg!("=== Application started ===");

    let network_state = Arc::new(Mutex::new(NetworkState::default()));

    let state_clone = Arc::clone(&network_state);
    let handle = thread::spawn(move || {
        background_updater(state_clone);
    });

    unsafe {
        BG_THREAD_HANDLE = Some(handle);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(network_state)
        .invoke_handler(tauri::generate_handler![
            get_network_stats,
            get_public_ip,
            get_weather,
            test_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
