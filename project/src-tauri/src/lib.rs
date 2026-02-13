use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// Simple logging macro - always enabled
macro_rules! log_msg {
    ($($arg:tt)*) => {
        {
            let msg = format!($($arg)*);
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .write(true)
                .open("D:\\code\\network-stats.log") {
                let _ = writeln!(file, "{}", msg);
            }
        }
    };
}

#[derive(Serialize, Clone)]
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
    last_update: Option<Instant>,
    last_latency: u32,
    last_packet_loss: f64,
    accumulated_received: u64,
    accumulated_sent: u64,
    last_download_speed: f64,
    last_upload_speed: f64,
    // Cached PowerShell output
    cached_received: u64,
    cached_sent: u64,
    cache_time: Option<Instant>,
    cache_valid_duration: Duration,
}

// Cache PowerShell result for 5 seconds
const CACHE_DURATION_SECS: u64 = 5;

// Get network stats with caching to avoid frequent PowerShell calls
#[cfg(target_os = "windows")]
fn get_network_bytes_cached(state: &mut NetworkState) -> (u64, u64) {
    let now = Instant::now();

    // Check if cache is still valid
    if let Some(cache_time) = state.cache_time {
        if now.duration_since(cache_time) < state.cache_valid_duration {
            log_msg!("Using cached network stats");
            return (state.cached_received, state.cached_sent);
        }
    }

    // Cache expired, fetch new data
    log_msg!("Fetching fresh network stats from PowerShell");

    let script = r#"
    $ErrorActionPreference = 'SilentlyContinue'
    $adapters = Get-NetAdapter | Where-Object { $_.Status -eq 'Up' }
    $totalReceived = 0L
    $totalSent = 0L
    foreach ($adapter in $adapters) {
        $stats = Get-NetAdapterStatistics -Name $adapter.Name -ErrorAction SilentlyContinue
        if ($stats) {
            $totalReceived += $stats.ReceivedBytes
            $totalSent += $stats.SentBytes
        }
    }
    Write-Output "$totalReceived,$totalSent"
    "#;

    let output = std::process::Command::new("powershell")
        .args([
            "-WindowStyle", "Hidden",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ])
        .creation_flags(0x08000000)
        .output();

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let trimmed = stdout.trim();

            log_msg!("PS output: {}", trimmed);

            if let Some(pos) = trimmed.find(',') {
                let received = trimmed[..pos].trim().parse::<u64>().unwrap_or(0);
                let sent = trimmed[pos + 1..].trim().parse::<u64>().unwrap_or(0);

                // Update cache
                state.cached_received = received;
                state.cached_sent = sent;
                state.cache_time = Some(now);

                log_msg!("Cached: recv={}, sent={}", received, sent);
                return (received, sent);
            }
        }
        Err(e) => {
            log_msg!("PowerShell failed: {}", e);
        }
    }

    // Return cached values even if stale
    (state.cached_received, state.cached_sent)
}

#[cfg(not(target_os = "windows"))]
fn get_network_bytes_cached(_state: &mut NetworkState) -> (u64, u64) {
    (0, 0)
}

#[cfg(target_os = "windows")]
fn ping_gateway(latency_samples: &mut Vec<u32>, packet_loss_samples: &mut Vec<bool>) -> Result<(u32, f64), String> {
    use std::process::Command;

    log_msg!("Pinging gateway...");

    // First try to get default gateway
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
            log_msg!("No gateway found, using 8.8.8.8");
            "8.8.8.8".to_string()
        }
    } else {
        log_msg!("Failed to get gateway, using 8.8.8.8");
        "8.8.8.8".to_string()
    };

    // Now ping the target
    let output = Command::new("ping")
        .args(["-n", "1", "-w", "2000", &target_ip])
        .creation_flags(0x08000000)
        .output();

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            log_msg!("Ping output: {}", stdout.trim());

            let lost = stdout.contains("Destination host unreachable")
                || stdout.contains("Request timed out")
                || stdout.contains("General failure")
                || stdout.contains("Ping request could not find")
                || stdout.contains("100% loss");

            packet_loss_samples.push(lost);
            log_msg!("Packet lost: {}", lost);

            if !lost {
                for line in stdout.lines() {
                    if line.contains("time=") || line.contains("time<") {
                        if let Some(time_part) = line.split("time=").nth(1).or_else(|| line.split("time<").nth(1)) {
                            if let Some(ms_str) = time_part.split("ms").next() {
                                if let Ok(latency) = ms_str.trim().parse::<f64>() {
                                    latency_samples.push(latency as u32);
                                    log_msg!("Latency: {}", latency);
                                    return Ok((latency as u32, 0.0));
                                }
                            }
                        }
                    }
                }
                if stdout.contains("bytes=") && stdout.contains("TTL=") {
                    latency_samples.push(1);
                    log_msg!("Latency: <1ms");
                    return Ok((1, 0.0));
                }
            }

            Ok((0, 0.0))
        }
        Err(e) => {
            log_msg!("Ping failed: {}", e);
            Ok((0, 0.0))
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn ping_gateway(latency_samples: &mut Vec<u32>, packet_loss_samples: &mut Vec<bool>) -> Result<(u32, f64), String> {
    latency_samples.push(30);
    packet_loss_samples.push(false);
    Ok((30, 0.0))
}

fn calculate_speed(
    _state: &NetworkState,
    current_received_delta: u64,
    current_sent_delta: u64,
    elapsed: f64,
) -> (f64, f64) {
    if elapsed < 0.1 {
        return (0.0, 0.0);
    }

    // Return speeds in KB/s
    let download_speed = (current_received_delta as f64 / elapsed) / 1024.0;
    let upload_speed = (current_sent_delta as f64 / elapsed) / 1024.0;

    // Cap at reasonable values (100 MB/s = 102400 KB/s)
    let download_speed = download_speed.min(102400.0);
    let upload_speed = upload_speed.min(102400.0);

    log_msg!("Speed: DL={:.2} KB/s, UL={:.2} KB/s", download_speed, upload_speed);

    (download_speed, upload_speed)
}

#[tauri::command]
fn get_network_stats(
    state: tauri::State<Arc<Mutex<NetworkState>>>,
    latency_samples: tauri::State<Arc<Mutex<Vec<u32>>>>,
    packet_loss_samples: tauri::State<Arc<Mutex<Vec<bool>>>>,
) -> Result<NetworkStats, String> {
    log_msg!("=== get_network_stats called ===");

    let mut state_guard = state.lock().unwrap();
    let now = Instant::now();

    // Get network bytes (cached or fresh)
    let (current_received, current_sent) = get_network_bytes_cached(&mut state_guard);

    log_msg!("Current bytes: received={}, sent={}", current_received, current_sent);

    let (download_speed, upload_speed) = if let Some(last_time) = state_guard.last_update {
        let elapsed = now.duration_since(last_time).as_secs_f64();
        log_msg!("Elapsed: {:.2}s", elapsed);

        if elapsed > 0.5 {
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

            log_msg!("Delta: received={}, sent={}", delta_received, delta_sent);

            state_guard.accumulated_received += delta_received;
            state_guard.accumulated_sent += delta_sent;

            let speed = calculate_speed(&state_guard, state_guard.accumulated_received, state_guard.accumulated_sent, elapsed);

            state_guard.accumulated_received = 0;
            state_guard.accumulated_sent = 0;
            state_guard.last_bytes_received = current_received;
            state_guard.last_bytes_sent = current_sent;
            state_guard.last_update = Some(now);

            speed
        } else {
            log_msg!("Using cached speeds: DL={:.2}, UL={:.2}", state_guard.last_download_speed, state_guard.last_upload_speed);
            (state_guard.last_download_speed, state_guard.last_upload_speed)
        }
    } else {
        log_msg!("First call - initializing");
        state_guard.last_bytes_received = current_received;
        state_guard.last_bytes_sent = current_sent;
        state_guard.last_update = Some(now);
        // Initialize cache duration
        state_guard.cache_valid_duration = Duration::from_secs(CACHE_DURATION_SECS);
        (0.0, 0.0)
    };

    state_guard.last_download_speed = download_speed;
    state_guard.last_upload_speed = upload_speed;

    // Update latency and packet loss every 10 seconds
    let (latency, packet_loss) = if state_guard.last_update.unwrap().elapsed() > Duration::from_secs(10) {
        {
            let mut lat_samples = latency_samples.lock().unwrap();
            let mut pl_samples = packet_loss_samples.lock().unwrap();

            let _ = ping_gateway(&mut lat_samples, &mut pl_samples);

            let avg_latency = if !lat_samples.is_empty() {
                let sum: u32 = lat_samples.iter().sum();
                sum / lat_samples.len() as u32
            } else {
                0
            };

            let avg_packet_loss = if !pl_samples.is_empty() {
                let lost_count = pl_samples.iter().filter(|&&x| x).count();
                (lost_count as f64 / pl_samples.len() as f64) * 100.0
            } else {
                0.0
            };

            log_msg!("Avg latency: {}ms, packet loss: {:.1}%", avg_latency, avg_packet_loss);

            state_guard.last_latency = avg_latency;
            state_guard.last_packet_loss = avg_packet_loss;

            while lat_samples.len() > 10 {
                lat_samples.remove(0);
            }
            while pl_samples.len() > 10 {
                pl_samples.remove(0);
            }

            (avg_latency, avg_packet_loss)
        }
    } else {
        (state_guard.last_latency, state_guard.last_packet_loss)
    };

    let status = if latency == 0 {
        "检测中...".to_string()
    } else if latency > 100 || packet_loss > 5.0 {
        "较差".to_string()
    } else if latency > 50 || packet_loss > 2.0 {
        "一般".to_string()
    } else {
        "良好".to_string()
    };

    log_msg!("Returning: latency={}, DL={:.2} KB/s, UL={:.2} KB/s, status={}", latency, download_speed, upload_speed, status);

    Ok(NetworkStats {
        latency,
        download_speed,
        upload_speed,
        packet_loss,
        status,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    log_msg!("=== Application started ===");

    let network_state = Arc::new(Mutex::new(NetworkState {
        cache_valid_duration: Duration::from_secs(CACHE_DURATION_SECS),
        ..Default::default()
    }));
    let latency_samples: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
    let packet_loss_samples: Arc<Mutex<Vec<bool>>> = Arc::new(Mutex::new(Vec::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(network_state)
        .manage(latency_samples)
        .manage(packet_loss_samples)
        .invoke_handler(tauri::generate_handler![get_network_stats])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
