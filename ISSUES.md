# 网络统计 - 问题记录与解决方案

## 问题汇总

### 1. IP地址获取失败

**问题描述**：
- IP地址显示为 "--"
- 所有HTTPS API请求失败（SSL/TLS错误）

**原因分析**：
- api.ipify.org 和 ifconfig.me 等API请求失败
- 可能是网络环境或SSL证书问题

**解决方案**：
- 使用PowerShell的System.Net.WebClient代替reqwest
- 添加多个备用API：api.ipify.org、ifconfig.me、myip.ipip.net
- 强制使用TLS 1.2

---

### 2. 延迟显示为0ms，丢包率100%

**问题描述**：
- 网络延迟一直显示0ms
- 丢包率显示100%

**原因分析**：
- `ping_gateway_internal()` 函数被正确调用并解析出了延迟值
- 但在UI更新时使用的是 `current_stats.latency`（初始值为0）
- 原因：延迟更新和速度更新共用同一个时间变量，导致延迟计算错误

**解决方案**：
- 添加独立的 `last_latency_update` 字段
- 延迟每10秒更新一次，速度每秒更新一次

---

### 3. 天气城市显示不正确

**问题描述**：
- 城市显示为"Dongqiao"而不是"Tianjin"

**原因分析**：
- wttr.in API返回的是具体区县（如东丽区 Dongqiao）而不是城市名

**解决方案**：
- 使用请求参数中的城市名而不是API返回的areaName

---

### 4. IP地址所在地时间没有显示

**问题描述**：
- 时间显示为 "--:--"

**原因分析**：
- wttr.in API返回的timezone字段可能为空
- `get_local_time_24h()` 函数没有正确返回时间

**解决方案**：
- 直接使用系统时间计算中国时间（UTC+8）

---

### 5. 窗口显示不完整（标题栏被截断）

**问题描述**：
- 窗口顶部只显示一半
- 窗口显示在屏幕边缘之外

**解决方案**：
- 在tauri.conf.json中设置初始窗口位置

---

### 6. 窗口高度不适配内容

**问题描述**：
- 窗口高度固定，内容显示不全
- 每次添加新功能都需要手动调整高度

**解决方案**：
- 使用JavaScript动态调整窗口高度

---

### 7. 天气城市和时间位置重叠

**问题描述**：
- 城市名称和时间显示重叠

**解决方案**：
- 将时间放在天气组件正中间，使用CSS定位

---

### 8. 网络恢复后不自动刷新

**问题描述**：
- 离线时打开应用没有IP和天气
- 连接网络后不会自动刷新

**解决方案**：
- 使用navigator.onLine监听网络状态变化

---

### 9. IP提取函数无法处理编码问题

**问题描述**：
- myip.ipip.net返回的中文响应有编码问题
- IP提取失败

**解决方案**：
- 重写extract_ip函数，逐字符扫描查找IP地址模式

---

### 10. IP地理位置获取失败，使用默认城市

**问题描述**：
- IP在Oregon，但显示的默认城市是New York
- 日志显示IP获取成功但city/country为空

**原因分析**：
- v14及之前版本：api.ipify.org只返回纯IP，没有地理位置信息
- 由于它排在API列表第一位，成功后就直接返回了，没有继续尝试JSON API
- v15版本：PowerShell脚本字符串替换错误，`.replace("{$URL}", url)` 不工作

**解决方案**：
- v15：将JSON API（ip-api.com, ipapi.co）放在列表前面优先尝试
- v16：修复PowerShell脚本构建，使用`format!`宏正确插入URL
- 从IP API获取timezone信息并传递给天气模块

---

### 11. 时间显示为本地时间而非IP所在地时间

**问题描述**：
- IP在美国Oregon，但显示的时间是中国时间
- 应该显示Oregon当地时间

**原因分析**：
- wttr.in天气API不返回timezone信息
- 之前代码从weather API获取timezone，但为空，所以使用了默认的中国时间

**解决方案**：
- IPInfo结构体添加`timezone`字段
- 从IP API (ip-api.com) 获取timezone（如America/Los_Angeles）
- 前端把timezone传给`get_weather`命令
- 后端使用传入的timezone计算当地时间

---

### 12. 日志文件无限增长

**问题描述**：
- network-stats.log文件越来越大
- 长时间运行后文件可达数十MB

**解决方案**：
- 添加日志滚动功能
- 只保留最开始200行和最新200行
- 每100条日志检查一次是否需要滚动

---

## 版本历史

| 版本 | 日期 | 主要更新 |
|------|------|----------|
| v5 | 2026-02-13 | 修复城市显示和时间显示 |
| v6 | 2026-02-13 | 添加CSS时间样式 |
| v7 | 2026-02-13 | 修复城市名为Tianjin |
| v8 | 2026-02-14 | 修复窗口位置 |
| v9 | 2026-02-14 | 添加自动调整窗口高度和网络检测 |
| v10 | 2026-02-14 | 调整窗口高度和时间位置 |
| v11 | 2026-02-14 | 时间居中显示 |
| v12 | 2026-02-14 | 支持获取国外城市天气 |
| v13 | 2026-02-14 | 修复国外城市解析，显示默认城市标记 |
| v14 | 2026-02-14 | 日志滚动保留：只保留前后各200行；修复国外时间：根据时区显示当地时间；美国IP显示州名 |
| v15 | 2026-02-14 | 尝试修复IP地理位置API（优先JSON API） |
| v16 | 2026-02-14 | 修复PowerShell脚本问题：format!宏使用错误 |
| v17 | 2026-02-14 | **IP所在地时间显示**：从IP API获取timezone并传给天气模块，正确显示Oregon当地时间 |

---

## 项目结构

```
floating-stats/
├── project/
│   ├── src/                 # 前端代码
│   │   ├── index.html       # HTML结构
│   │   ├── main.js          # JavaScript逻辑
│   │   └── styles.css       # 样式
│   └── src-tauri/           # Rust后端
│       ├── src/
│       │   └── lib.rs       # 主要逻辑
│       ├── Cargo.toml       # 依赖配置
│       └── tauri.conf.json  # Tauri配置
└── floating-stats-v17.exe   # 最新版本
```

---

## API说明

### IP地理位置API

**首选：ip-api.com**
- URL: `http://ip-api.com/json/`
- 返回：IP, country, regionName, city, timezone
- 免费，限制45req/min
- 示例响应：
```json
{
  "status": "success",
  "country": "United States",
  "regionName": "Oregon",
  "city": "The Dalles",
  "timezone": "America/Los_Angeles",
  "query": "34.105.5.167"
}
```

**备用：ipapi.co**
- URL: `https://ipapi.co/json/`
- 返回类似信息

**纯IP备用：api.ipify.org**
- URL: `https://api.ipify.org`
- 只返回IP地址，无地理位置

### 天气API

**wttr.in**
- URL: `https://wttr.in/{城市}?format=j1&lang=zh`
- 返回：温度、天气描述、地区信息
- 注意：不返回timezone信息

---

## 时区支持

当前支持的时区映射：

| 地区 | 时区 | 偏移 |
|------|------|------|
| 中国 | Asia/Shanghai, China | UTC+8 |
| 日本 | Asia/Tokyo | UTC+9 |
| 韩国 | Asia/Seoul | UTC+9 |
| 美国东部 | America/New_York, EST, EDT | UTC-5/-4 |
| 美国太平洋 | America/Los_Angeles, PST, PDT | UTC-8/-7 |
| 美国中部 | America/Chicago, CST, CDT | UTC-6/-5 |
| 美国山区 | America/Denver, MST, MDT | UTC-7/-6 |
| 英国 | Europe/London, GMT, BST | UTC+0/+1 |
| 欧洲大陆 | Europe/Paris, Europe/Berlin | UTC+1/+2 |
| 俄罗斯 | Europe/Moscow | UTC+3 |
| 澳大利亚 | Australia/Sydney, Australia/Melbourne | UTC+11 |
| 新西兰 | Pacific/Auckland | UTC+13 |
| 阿联酋 | Asia/Dubai | UTC+4 |
