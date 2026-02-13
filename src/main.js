const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

// 格式化速度显示 - 现在用 KB/s
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
const weatherIcons = {
  '晴': '☀️',
  '多云': '⛅',
  '阴': '☁️',
  '雨': '🌧️',
  '雪': '❄️',
  '雷': '⛈️',
  '雾': '🌫️',
  '霾': '😷',
  '风': '💨',
};

// 天气状态码映射（和风天气）
const weatherCodeMap = {
  // 晴
  100: '晴', 150: '晴',
  // 多云
  101: '多云', 102: '多云', 103: '多云',
  // 阴
  104: '阴',
  // 雨
  300: '雨', 301: '雨', 302: '小雨', 303: '中雨', 304: '大雨',
  305: '暴雨', 306: '大暴雨', 307: '特大暴雨', 308: '毛毛雨',
  309: '小雨', 310: '雨', 311: '中雨', 312: '大雨',
  313: '暴雨', 314: '暴雨', 315: '大雨', 316: '中雨',
  317: '小雨', 318: '雨', 350: '雨', 351: '雨',
  // 雪
  400: '雪', 401: '小雪', 402: '中雪', 403: '大雪', 404: '暴雪',
  405: '大雪', 406: '中雪', 407: '小雪', 408: '小雪', 409: '中雪',
  410: '雪', 456: '雨夹雪', 457: '雨夹雪',
  // 雷
  500: '雷', 501: '雷', 502: '雷', 503: '雷', 504: '雷',
  507: '雷', 508: '雷', 509: '雷', 510: '雷', 511: '雷',
  512: '雷', 513: '雷', 514: '雷', 515: '雷',
  // 雾霾
  800: '雾', 801: '雾', 802: '雾', 803: '雾', 804: '雾',
  805: '雾', 806: '雾', 807: '雾',
  900: '霾', 901: '霾',
  // 风
  200: '风',
};

function getWeatherIcon(code) {
  const desc = weatherCodeMap[code] || '晴';
  return weatherIcons[desc] || '🌤️';
}

function getWeatherDesc(code) {
  return weatherCodeMap[code] || '晴';
}

// 自动获取城市（通过 IP）
async function getLocation() {
  try {
    const response = await fetch('https://ipapi.co/json/');
    const data = await response.json();
    return {
      city: data.city || '未知',
      lat: data.latitude,
      lon: data.longitude,
      country: data.country_name || ''
    };
  } catch (error) {
    console.error('获取位置失败:', error);
    // 返回默认位置（北京）
    return {
      city: '北京',
      lat: 39.9042,
      lon: 116.4074,
      country: '中国'
    };
  }
}

// 获取天气（使用和风天气免费版）
// 需要 API key，这里使用公开的测试接口或使用 wttr.in
async function getWeather() {
  try {
    // 先获取位置
    const location = await getLocation();

    // 使用 wttr.in 免费天气 API（无需 key）
    const response = await fetch(`https://wttr.in/${encodeURIComponent(location.city)}?format=j1`);
    if (!response.ok) {
      throw new Error('天气 API 请求失败');
    }
    const data = await response.json();

    // 解析 wttr.in 数据
    const current = data.current_condition[0];
    const area = data.nearest_area[0];

    const temp = current.temp_C;
    const desc = current.weatherDesc[0].value;
    const locationName = area.areaName[0].value;

    // 根据天气描述选择图标
    let icon = '🌤️';
    const descLower = desc.toLowerCase();
    if (descLower.includes('sunny') || descLower.includes('clear')) {
      icon = '☀️';
    } else if (descLower.includes('cloudy') || descLower.includes('overcast')) {
      icon = '☁️';
    } else if (descLower.includes('partly')) {
      icon = '⛅';
    } else if (descLower.includes('rain') || descLower.includes('drizzle') || descLower.includes('shower')) {
      icon = '🌧️';
    } else if (descLower.includes('snow') || descLower.includes('sleet')) {
      icon = '❄️';
    } else if (descLower.includes('thunder') || descLower.includes('storm')) {
      icon = '⛈️';
    } else if (descLower.includes('fog') || descLower.includes('mist')) {
      icon = '🌫️';
    }

    return {
      temp: parseInt(temp),
      desc: desc,
      location: locationName,
      icon: icon
    };
  } catch (error) {
    console.error('获取天气失败:', error);
    return {
      temp: '--',
      desc: '获取失败',
      location: '--',
      icon: '❓'
    };
  }
}

// 更新天气显示
async function updateWeather() {
  try {
    const weather = await getWeather();

    document.getElementById('weatherTemp').textContent = `${weather.temp}°C`;
    document.getElementById('weatherDesc').textContent = weather.desc;
    document.getElementById('weatherLocation').textContent = weather.location;
    document.getElementById('weatherIcon').textContent = weather.icon;
  } catch (error) {
    console.error('更新天气失败:', error);
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
  updateWeather();

  // 设置定时更新（每 5 秒更新一次，减少 PowerShell 调用）
  setInterval(updateStats, 5000);

  // 天气每 10 分钟更新一次
  setInterval(updateWeather, 10 * 60 * 1000);

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
