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

  // 设置定时更新（每 5 秒更新一次，减少 PowerShell 调用）
  setInterval(updateStats, 5000);

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
