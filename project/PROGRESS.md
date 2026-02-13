# 网络统计 - 项目进度

## 待办
- [ ] 实现 Windows API 直接获取网络统计（避免 PowerShell 慢）
- [ ] 修复 ping 失败问题，显示真实延迟
- [ ] 成功生成 MSI 安装包

## 已完成
- [x] 基础 Tauri 项目搭建
- [x] 添加透明度控制滑块
- [x] 修复窗口大小问题（添加 wrapper 居中）
- [x] 速度单位改为 KB/s，提高精度
- [x] 添加日志功能
- [x] 添加 5 秒缓存减少 PowerShell 调用
- [x] 创建独立项目文件夹 `D:\FloatingStats\`

## 当前问题
- **MSI 打包被阻止** - `light.exe` 被 Windows 安全拦截，尝试注册表添加排除项未验证是否生效
- **网络延迟一直为 0** - ping 命令执行失败或解析失败（需要查看日志 `D:\code\network-stats.log`）
- **实时性差** - PowerShell 调用导致 UI 卡顿，需要改用 Windows API 或后台线程

## 文件位置
- **源代码**: `D:\FloatingStats\project\`
- **可运行程序**: `D:\FloatingStats\FloatingStats.exe`
- **日志文件**: `D:\code\network-stats.log`
