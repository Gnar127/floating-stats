const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

// 动态调整窗口高度
async function adjustWindowHeight() {
  const widget = document.getElementById('widget');
  const scrollHeight = widget.scrollHeight;
  const currentWindow = getCurrentWindow();

  // 设置窗口高度为内容高度 + 一些边距
  await currentWindow.setSize({ type: 'Physical', width: 280, height: scrollHeight });
}

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
  if (t.includes('partly') || t.includes('partly') || t.includes('cloudy') || t.includes('多云')) return '⛅';
  if (t.includes('rain') || t.includes('drizzle') || t.includes('shower') || t.includes('雨')) return '🌧️';
  if (t.includes('snow') || t.includes('sleet') || t.includes('雪')) return '❄️';
  if (t.includes('thunder') || t.includes('storm') || t.includes('雷')) return '⛈️';
  if (t.includes('fog') || t.includes('mist') || t.includes('雾')) return '🌫️';
  return '🌤️';
}

// 格式化时间为24小时制
function formatTime24(date) {
  const hours = date.getHours().toString().padStart(2, '0');
  const minutes = date.getMinutes().toString().padStart(2, '0');
  return `${hours}:${minutes}`;
}

// 获取 IP 信息 - 使用后端命令，带错误处理和重试
async function getIPInfo() {
  console.log('Fetching IP via backend...');
  try {
    const ipInfo = await invoke('get_public_ip');
    console.log('IP info received:', ipInfo);
    // 确保返回完整的IP信息，包括城市、国家和时区
    return {
      ip: ipInfo.ip || '--',
      city: ipInfo.city || '未知',
      country: ipInfo.country || '--',
      timezone: ipInfo.timezone || ''
    };
  } catch (error) {
    console.error('获取IP失败:', error);
    return {
      ip: '--',
      city: '未知',
      country: '--',
      timezone: ''
    };
  }
}

// 获取天气信息 - 使用后端命令，接收时区参数
async function getWeatherInfo(city, timezone) {
  console.log('Fetching weather via backend for:', city, 'timezone:', timezone);
  try {
    const weather = await invoke('get_weather', { city, timezone });
    console.log('Weather info received:', weather);
    return weather;
  } catch (error) {
    console.error('获取天气失败:', error);
    return {
      temp: '--°C',
      desc: '获取失败',
      location: city,
      country: '--',
      local_time: '--:--',
      icon: '❓'
    };
  }
}

// 更新天气和 IP 显示
async function updateWeatherAndIP() {
  try {
    console.log('=== Starting weather and IP update ===');

    // 获取 IP 信息
    const ipInfo = await getIPInfo();
    document.getElementById('ipAddress').textContent = ipInfo.ip;

    console.log('IP Info:', ipInfo);

    // 根据 IP 的城市来获取天气
    // ipInfo.city 应该是从IP API返回的真实城市名（支持全球城市）
    let weatherCity = ipInfo.city;
    let isDefaultCity = false;

    if (ipInfo.city === '本地' || ipInfo.city === '未知' || !ipInfo.city || ipInfo.city === 'Unknown') {
      // 根据国家选择默认城市
      if (ipInfo.country === 'China' || ipInfo.country === '中国') {
        weatherCity = 'Beijing';
      } else {
        // 国外默认使用纽约
        weatherCity = 'New York';
      }
      isDefaultCity = true;
    }

    console.log('Fetching weather for city:', weatherCity, 'country:', ipInfo.country, 'isDefault:', isDefaultCity, 'timezone:', ipInfo.timezone);
    const weather = await getWeatherInfo(weatherCity, ipInfo.timezone);

    // 更新 UI
    document.getElementById('weatherTemp').textContent = weather.temp;
    document.getElementById('weatherDesc').textContent = weather.desc;

    // 显示城市和国家
    let locationText = weather.location;
    if (isDefaultCity) {
      // 默认城市显示 (默认)
      locationText = weather.location + ' (默认)';
    } else if (ipInfo.country && ipInfo.country !== '--' && ipInfo.country !== 'China' && ipInfo.country !== '中国') {
      locationText = weather.location + ', ' + ipInfo.country;
    } else if (ipInfo.country === 'China' || ipInfo.country === '中国') {
      locationText = weather.location + ' (中国)';
    }
    document.getElementById('weatherLocation').textContent = locationText;

    document.getElementById('weatherIcon').textContent = weather.icon;
    document.getElementById('locationTime').textContent = weather.local_time;

    console.log('更新完成:', { ip: ipInfo.ip, city: weatherCity, weather });
  } catch (error) {
    console.error('更新天气/IP失败:', error);
    document.getElementById('weatherDesc').textContent = '网络错误';
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

    // 更新时间 - 24小时制
    const now = new Date();
    const timeStr = formatTime24(now);
    document.getElementById('updateTime').textContent = timeStr;
  } catch (error) {
    console.error('Failed to get network stats:', error);
  }
}

// 检测网络状态变化
let wasOnline = navigator.onLine;
let ipRefreshTimer = null;

function checkNetworkChange() {
  const isOnline = navigator.onLine;

  if (!wasOnline && isOnline) {
    console.log('网络已连接，刷新IP和天气...');
    // 网络从离线变为在线，立即刷新
    updateWeatherAndIP();
  } else if (isOnline && ipRefreshTimer) {
    // 清除之前的定时器
    clearTimeout(ipRefreshTimer);
    // 设置新的定时器，5秒后刷新（防止频繁刷新）
    ipRefreshTimer = setTimeout(() => {
      updateWeatherAndIP();
    }, 5000);
  }

  wasOnline = isOnline;
}

// 初始化
window.addEventListener("DOMContentLoaded", async () => {
  console.log('=== DOMContentLoaded, initializing app ===');

  // 测试 Tauri 命令系统
  try {
    const testResult = await invoke('test_command');
    console.log('Test command result:', testResult);
  } catch (e) {
    console.error('Test command failed:', e);
  }

  // 初始更新
  updateStats();
  updateWeatherAndIP();

  // 等待一下让内容渲染完成，然后调整窗口高度
  setTimeout(adjustWindowHeight, 500);

  // 网络速度：每 1 秒更新一次
  setInterval(updateStats, 1000);

  // 天气和 IP 每 10 分钟更新一次
  setInterval(updateWeatherAndIP, 10 * 60 * 1000);

  // 监听网络状态变化
  window.addEventListener('online', checkNetworkChange);
  window.addEventListener('offline', checkNetworkChange);

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

  // 窗口大小变化时重新调整
  window.addEventListener('resize', () => {
    adjustWindowHeight();
  });

  console.log('=== App initialization complete ===');
});
