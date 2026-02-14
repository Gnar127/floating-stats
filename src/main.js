const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

// 格式化速度显示
function formatSpeed(speed) {
  if (speed < 1) {
    return (speed * 1024).toFixed(2) + ' B/s';
  }
  if (speed < 1024) {
    return speed.toFixed(2) + ' KB/s';
  }
  return (speed / 1024).toFixed(2) + ' MB/s';
}

// 天气图标映射
function getWeatherIcon(text) {
  const t = text.toLowerCase();
  if (t.includes('sunny') || t.includes('clear') || t.includes('晴')) return '☀️';
  if (t.includes('cloud') || t.includes('overcast') || t.includes('阴')) return '☁️';
  if (t.includes('partly') || t.includes('多云')) return '⛅';
  if (t.includes('rain') || t.includes('drizzle') || t.includes('shower') || t.includes('雨')) return '🌧️';
  if (t.includes('snow') || t.includes('sleet') || t.includes('雪')) return '❄️';
  if (t.includes('thunder') || t.includes('storm') || t.includes('雷')) return '⛈️';
  if (t.includes('fog') || t.includes('mist') || t.includes('雾')) return '🌫️';
  return '🌤️';
}

// 获取本地时间（根据时区）
function getLocalTime(timezone) {
  try {
    const now = new Date();
    return now.toLocaleTimeString('en-US', {
      timeZone: timezone,
      hour12: false,
      hour: '2-digit',
      minute: '2-digit'
    });
  } catch (e) {
    return '--:--';
  }
}

// 获取天气信息（使用 wttr.in，支持中文城市名）
async function getWeatherInfo(city) {
  try {
    const response = await fetch(`https://wttr.in/${encodeURIComponent(city)}?format=j1`);
    if (!response.ok) {
      throw new Error('天气 API 请求失败');
    }

    const data = await response.json();

    // 解析 wttr.in 数据
    const current = data.current_condition[0];
    const area = data.nearest_area[0];

    const temp = parseInt(current.temp_C);
    const desc = current.weatherDesc[0].value;
    const locationName = area.areaName[0].value;
    const country = area.country[0].value;
    const timezone = area.timezone[0].value;

    // 计算当地时间
    const localTime = getLocalTime(timezone);

    return {
      temp,
      desc,
      location: locationName,
      country,
      localTime,
      icon: getWeatherIcon(desc)
    };
  } catch (error) {
    console.error('获取天气失败:', error);
    return {
      temp: '--',
      desc: '获取失败',
      location: '--',
      country: '--',
      localTime: '--:--',
      icon: '❓'
    };
  }
}

// 获取 IP 信息
async function getIPInfo() {
  try {
    // 使用多个 API 提高成功率
    const apis = [
      'https://ipapi.co/json/',
      'https://api.ipify.org?format=json',
      'https://ip.sb/api/'
    ];

    for (const api of apis) {
      try {
        const response = await fetch(api);
        if (response.ok) {
          const data = await response.json();
          return {
            ip: data.ip || data.query || '--',
            city: data.city || data.region || '未知',
            country: data.country_name || data.country || '--'
          };
        }
      } catch (e) {
        continue;
      }
    }

    throw new Error('所有 IP API 都失败');
  } catch (error) {
    console.error('获取IP失败:', error);
    return {
      ip: '--',
      city: '未知',
      country: '--'
    };
  }
}

// 更新天气和 IP 显示
async function updateWeatherAndIP() {
  try {
    const ipInfo = await getIPInfo();

    // 如果获取到了城市，用真实城市；否则用默认城市
    const weather = await getWeatherInfo(ipInfo.city || 'Beijing');

    document.getElementById('weatherTemp').textContent = `${weather.temp}°C`;
    document.getElementById('weatherDesc').textContent = weather.desc;
    document.getElementById('weatherLocation').textContent = `${weather.location} (${weather.country})`;
    document.getElementById('weatherIcon').textContent = weather.icon;
    document.getElementById('locationTime').textContent = weather.localTime;
    document.getElementById('ipAddress').textContent = ipInfo.ip;

    console.log('更新完成:', { ip: ipInfo.ip, city: ipInfo.city, weather });
  } catch (error) {
    console.error('更新天气/IP失败:', error);
  }
}

// 更新 UI
async function updateStats() {
  try {
    const stats = await invoke('get_network_stats');

    document.getElementById('latency').textContent = `${stats.latency} ms`;
    document.getElementById('download').textContent = formatSpeed(stats.download_speed);
    document.getElementById('upload').textContent = formatSpeed(stats.upload_speed);
    document.getElementById('packetLoss').textContent = `${stats.packet_loss.toFixed(1)} %`;

    const statusEl = document.getElementById('status');
    statusEl.textContent = stats.status;
    statusEl.className = 'stat-value status';

    if (stats.status === '一般') {
      statusEl.classList.add('warning');
    } else if (stats.status === '较差') {
      statusEl.classList.add('error');
    }

    // 更新时间
    const now = new Date();
    const timeStr = now.toLocaleTimeString('zh-CN', { hour12: false });
    document.getElementById('updateTime').textContent = timeStr;
  } catch (error) {
    console.error('Failed to get network stats:', error);
  }
}

// 初始化
window.addEventListener("DOMContentLoaded", () => {
  // 初始更新
  updateStats();
  updateWeatherAndIP();

  // 设置定时更新（每 5 秒更新一次）
  setInterval(updateStats, 5000);

  // 天气和 IP 每 10 分钟更新一次
  setInterval(updateWeatherAndIP, 10 * 60 * 1000);

  // 关闭按钮
  document.getElementById('closeBtn').addEventListener('click', () => {
    getCurrentWindow().close();
  });

  // 透明度滑块
  const slider = document.getElementById('transparencySlider');
  const widget = document.getElementById('widget');

  slider.addEventListener('input', (e) => {
    const value = e.target.value;
    const opacity = value / 100;
    widget.style.background = `linear-gradient(135deg, rgba(30, 30, 50, ${opacity}) 0%, rgba(20, 20, 35, ${opacity}) 100%)`;
  });
});
