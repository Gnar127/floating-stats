import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

// 每 50 次对话后提醒重新打开
const CHAT_LIMIT = 50;

let messageCount = 0;

async function checkAndRemind() {
  messageCount++;

  if (messageCount >= CHAT_LIMIT)) {
    const stats = await invoke('get_network_stats');
    const shouldRemind = confirm(
      `已对话 ${messageCount} 次，建议重新打开以节省 token。\n\n` +
      `当前网络延迟: ${stats.latency}ms\n` +
      `下载速度: ${stats.download_speed.toFixed(2)} KB/s\n` +
      `丢包率: ${stats.packet_loss.toFixed(1)}%`
    );

    if (shouldRemind) {
      messageCount = 0;
    await getCurrentWindow().close();
    }
  }
}

// 页面加载后每条消息都计数
window.addEventListener('DOMContentLoaded', () => {
  // 覆盖 invoke 来计数消息
  const originalInvoke = window.__TAURI__.core.invoke;
  window.__TAURI__.core.invoke = async (...args) => {
    checkAndRemind();
    return originalInvoke(...args);
  };

  // 初始化计数
  checkAndRemind();
});
