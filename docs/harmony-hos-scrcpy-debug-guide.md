# HarmonyOS `hosScrcpy` / `UITest Agent` 调试与开发指南

## 目标

这份文档只面向正向开发，关注官方分发的 `hosScrcpy`、`uitest_agent`、`xdevice-devicetest` 相关能力，回答四个问题：

1. 官方二进制和控制层代码在哪里
2. `scrcpy server` 与 `uitest agent` 分别负责什么
3. 设备上如何拉起、观察和调试这些 so
4. `hscrcpy` 后续参考实现时，应该优先走哪条路线

## 结论

截至 `2026-04-24`，已经可以高置信度确认：

- 华为官方分发包中直接包含 `scrcpy server` 与 `uitest agent` 设备侧二进制
- `scrcpy server` 负责 `H.264` 投屏主链路，核心模型是：
  - `uitest extension`
  - `虚拟屏`
  - `平台 AVC 编码`
  - `gRPC / Unix socket`
- `uitest agent` 负责 `UITest/Hypium` 能力，核心模型是：
  - UI 树抓取
  - 手势与控件操作
  - 连续 JPEG 抓屏
- 这两条能力路线都不依赖普通应用层 `AVScreenCapture` 授权弹窗模型

## 官方分发物位置

### 本地下载包

当前已确认的本地官方包路径：

- [`/Users/magicdian/Downloads/devecotesting-hypium-6.1.0.210`](/Users/magicdian/Downloads/devecotesting-hypium-6.1.0.210)

关键文件：

- [`/Users/magicdian/Downloads/devecotesting-hypium-6.1.0.210/DevecoTesting-Hypium/lib/hosScrcpy-1.0.15-beta.jar`](/Users/magicdian/Downloads/devecotesting-hypium-6.1.0.210/DevecoTesting-Hypium/lib/hosScrcpy-1.0.15-beta.jar)
- [`/Users/magicdian/Downloads/devecotesting-hypium-6.1.0.210/DevecoTesting-Hypium/lib/DevecoTesting-Hypium-6.1.0.210.jar`](/Users/magicdian/Downloads/devecotesting-hypium-6.1.0.210/DevecoTesting-Hypium/lib/DevecoTesting-Hypium-6.1.0.210.jar)
- [`/Users/magicdian/Downloads/devecotesting-hypium-6.1.0.210/hypium-6.1.0.210/xdevice_devicetest-6.1.0.210-py3-none-any.whl`](/Users/magicdian/Downloads/devecotesting-hypium-6.1.0.210/hypium-6.1.0.210/xdevice_devicetest-6.1.0.210-py3-none-any.whl)

### 包内关键内容

`hosScrcpy-1.0.15-beta.jar` 和 `DevecoTesting-Hypium-6.1.0.210.jar` 中包含：

- `libscrcpy/libscrcpy_server0.z.so`
- `libscrcpy/libscrcpy_server1.z.so`
- `libscrcpy/libscrcpy_server2.z.so`
- `libscrcpy/libscrcpy_server3.z.so`
- `libscrcpy/libscrcpy_server_5.10-20260114.z.so`
- `libscrcpy/libscrcpy_server_emulator.z.so`
- `libscrcpy/libscrcpy_server_unix_6.3.1-20260113.z.so`
- `libscrcpy/libscrcpy_server_unix_6.4-20260113.z.so`
- `libscrcpy/libscrcpy_server_unix_6.5-20260313.z.so`
- `uitest_agent_1.1.12.so`
- `uitest_agent_1.1.3.so`
- `uitest_agent_1.1.5.so`
- `uitest_agent_1.2.3.so`
- `uitest_agent_x86_1.1.9.so`

`xdevice_devicetest-6.1.0.210-py3-none-any.whl` 中包含：

- `devicetest/controllers/tools/recorder/record_agent.py`
- `devicetest/controllers/tools/recorder/rpc_manager.py`
- `devicetest/controllers/tools/recorder/proto/scrcpy_pb2.py`
- `devicetest/controllers/tools/screen_agent.py`
- `devicetest/res/prototype/native/uitest_agent_*.so`
- `devicetest/res/recorder/libscrcpy_server*.z.so`

## 关键能力拆分

### `scrcpy server`

职责：

- 启动投屏会话
- 创建虚拟屏
- 绑定平台编码器 surface
- 输出 `H.264`
- 通过 `gRPC` 回传码流
- 响应 `RequestIDRFrame`

关键指纹：

- `xdevice_scrcpy`
- `Welcome to xdevice scrcpy so!`
- `scrcpy_grpc_socket`
- `CreateVirtualScreen`
- `video/avc`

典型依赖：

- `libgrpc.z.so`
- `libgrpcxx.z.so`
- `libgpr.z.so`
- `libprotobuf.z.so`
- `librender_service_base.z.so`
- `libnative_media_venc.so`
- `libnative_media_core.so`
- `libaudio_capturer.z.so`
- `libsurface.z.so`

### `uitest agent`

职责：

- `UITest/Hypium` 扩展入口
- UI 树抓取
- 控件查找
- 手势注入
- 连续 JPEG 抓屏

关键指纹：

- `Welcome to devicetest agent so!`
- `UiTestExtension_OnInit done`
- `com.ohos.devicetest.hypiumApiHelper`
- `uitest_socket`

典型依赖：

- `libutils.z.so`
- `libdm.z.so`
- `libapp_manager.z.so`
- `libsamgr_proxy.z.so`
- `libipc_single.z.so`
- `libbegetutil.z.so`

## 已确认的启动模型

### `H.264` 主链路

```text
desktop
  -> 推送 scrcpy server so
  -> uitest start-daemon singleness --extension-name <scrcpy so>
  -> 进程改名 xdevice_scrcpy
  -> 创建虚拟屏
  -> 配置平台 AVC 编码器
  -> 打开 scrcpy_grpc_socket
  -> gRPC 回传 H.264 access unit
```

### `UITest` / JPEG / UI 树链路

```text
desktop
  -> 推送 uitest agent so
  -> uitest start-daemon singleness
  -> 打开 uitest_socket
  -> 调用 Hypium helper
  -> captureLayout / startCaptureScreen / stopCaptureScreen
  -> 返回 UI 树或连续 JPEG 数据
```

## 官方控制层调用方式

### `record_agent.py`

官方 `xdevice-devicetest` 控制层里，录屏主路径是：

1. 推送：
   - `/data/local/tmp/libscreen_recorder.z.so`
2. 通过 `uitest` 拉起：
   - `/system/bin/uitest start-daemon singleness --extension-name libscreen_recorder.z.so -p {} -m 1 -screenId {}`
3. 建立转发：
   - `screen_record_grpc_socket`
4. 通过 gRPC stub 调用：
   - `onStart`
   - `onEnd`
   - `onRequestIDRFrame`

### `screen_agent.py`

官方 `screen_agent.py` 负责截图与屏幕采集相关逻辑，和 `UITest/Hypium` 截图链路配合使用。

## 如何解包官方分发物

```bash
mkdir -p /tmp/devecotesting_610210

python3 - <<'PY'
from zipfile import ZipFile
from pathlib import Path

files = [
    Path('/Users/magicdian/Downloads/devecotesting-hypium-6.1.0.210/DevecoTesting-Hypium/lib/hosScrcpy-1.0.15-beta.jar'),
    Path('/Users/magicdian/Downloads/devecotesting-hypium-6.1.0.210/DevecoTesting-Hypium/lib/DevecoTesting-Hypium-6.1.0.210.jar'),
    Path('/Users/magicdian/Downloads/devecotesting-hypium-6.1.0.210/hypium-6.1.0.210/xdevice_devicetest-6.1.0.210-py3-none-any.whl'),
]

for src in files:
    out = Path('/tmp/devecotesting_610210') / src.name
    out.mkdir(parents=True, exist_ok=True)
    with ZipFile(src) as zf:
        zf.extractall(out)
PY
```

## 本地检查命令

### 看版本与指纹

```bash
strings /tmp/devecotesting_610210/hosScrcpy-1.0.15-beta.jar/libscrcpy/libscrcpy_server_unix_6.5-20260313.z.so | \
  rg '6.5-20260313|xdevice_scrcpy|Welcome to xdevice scrcpy so!'

strings /tmp/devecotesting_610210/hosScrcpy-1.0.15-beta.jar/uitest_agent_1.2.3.so | \
  rg 'Welcome to devicetest agent so!|UiTestExtension_OnInit done'
```

### 看 ELF 依赖

```bash
objdump -p /tmp/devecotesting_610210/hosScrcpy-1.0.15-beta.jar/libscrcpy/libscrcpy_server_unix_6.5-20260313.z.so | rg 'SONAME|NEEDED'
objdump -p /tmp/devecotesting_610210/hosScrcpy-1.0.15-beta.jar/uitest_agent_1.2.3.so | rg 'SONAME|NEEDED'
```

### 看控制层代码

```bash
sed -n '1,260p' /tmp/devecotesting_610210/xdevice_devicetest-6.1.0.210-py3-none-any.whl/devicetest/controllers/tools/recorder/record_agent.py
sed -n '1,200p' /tmp/devecotesting_610210/xdevice_devicetest-6.1.0.210-py3-none-any.whl/devicetest/controllers/tools/recorder/rpc_manager.py
sed -n '1,240p' /tmp/devecotesting_610210/xdevice_devicetest-6.1.0.210-py3-none-any.whl/devicetest/controllers/tools/screen_agent.py
```

## 设备侧调试命令

### 看当前进程

```bash
DEVICE="$(hdc list targets | awk 'NR==1{print $1}')"
hdc -t "$DEVICE" shell 'ps -ef | grep -E "[x]device_scrcpy|[u]itest start-daemon|[h]scrcpy_poc"'
```

### 看当前 socket

```bash
DEVICE="$(hdc list targets | awk 'NR==1{print $1}')"
hdc -t "$DEVICE" shell 'cat /proc/net/unix | grep -E "scrcpy_grpc_socket|screen_record_grpc_socket|uitest_socket"'
```

### 看端口转发

```bash
hdc fport ls
```

### 看关键日志

```bash
DEVICE="$(hdc list targets | awk 'NR==1{print $1}')"
hdc -t "$DEVICE" shell 'hilog -x | grep -E "xdevice_scrcpy|scrcpy_grpc_socket|screen_record|uitest|hypium|UiTestKit_Addon|CreateVirtualScreen|video/avc"'
```

### 抓设备现场 so

```bash
DEVICE="$(hdc list targets | awk 'NR==1{print $1}')"
OUT="/tmp/device-extension-captures/$(date +%F-%H%M%S)"
mkdir -p "$OUT"

while true; do
  hdc -t "$DEVICE" shell 'test -f /data/local/tmp/agent.so' >/dev/null 2>&1 && \
    hdc -t "$DEVICE" file recv /data/local/tmp/agent.so "$OUT/agent.so" >/dev/null 2>&1

  hdc -t "$DEVICE" shell 'test -f /data/local/tmp/scrcpy_server.so' >/dev/null 2>&1 && \
    hdc -t "$DEVICE" file recv /data/local/tmp/scrcpy_server.so "$OUT/scrcpy_server.so" >/dev/null 2>&1

  if [ -f "$OUT/agent.so" ] && [ -f "$OUT/scrcpy_server.so" ]; then
    echo "captured into $OUT"
    ls -lh "$OUT"
    break
  fi

  sleep 0.1
done
```

## 已知运行时特征

### 典型启动参数

现场已观察到过的参数形态：

```bash
uitest start-daemon singleness \
  --extension-name scrcpy_server.so \
  -scale 1 \
  -frameRate 120 \
  -bitRate 31457280 \
  -p 9958 \
  -screenId 0 \
  -encodeType 0 \
  -iFrameInterval 2000 \
  -repeatInterval 33
```

可以先这样理解：

- `frameRate=120`
- `bitRate=31457280`，即 `30 Mbps`
- `iFrameInterval=2000`
- `repeatInterval=33`

### 生命周期

已经确认：

- `xdevice_scrcpy` 不是常驻服务
- 会话开始时由 `uitest` 临时拉起
- 会话结束后进程退出

这意味着后续自定义实现更合理的模型应当是：

- 会话型 extension

而不是：

- 后台常驻守护进程

### 自制 `uitest extension so` POC 当前结论

这一轮已经做过自制 extension so 对照实验，结果需要明确收口：

- 自制 `hscrcpy_uitest_poc.so` 已经具备：
  - `UiTestExtension_OnInit`
  - `UiTestExtension_OnRun`
- 也已经被打进 signed HAP
- 但直接推到 `/data/local/tmp` 后，`uitest` 会在装载阶段报：
  - `Xpm check failed`
  - `Dlopen ... Permission denied`

对照组里的官方 `uitest_agent_1.2.3.so`：

- 同样位于 `/data/local/tmp`
- 同样加了执行位
- 可以被 `uitest` 正常拉起

所以当前应当视为：

- blocker 不在 ABI 或导出符号
- blocker 更像是 `uitest` 的受信任 extension 装载校验
- 现阶段“自己写一个 so，直接通过 `uitest --extension-name` 使用”不可作为主开发路径

## 对 `hscrcpy` 的开发建议

当前可以把路线拆成三层：

1. 公开应用侧路线
   - `AVScreenCapture + 自己编码`
2. `uitest extension` 最小验证路线
   - 先验证自定义 so 是否可加载、可运行、可持有 socket
3. 官方 `hosScrcpy` 参考路线
   - 参考虚拟屏、参数、gRPC、IDR 请求和会话模型

当前最稳妥的推进顺序：

1. 保留现有 `AVScreenCapture` 主链路继续推进 host 解码渲染
2. 将 [`docs/uitest-extension-poc.md`](/Users/magicdian/Documents/personal_project/hscrcpy/docs/uitest-extension-poc.md) 保留为实验记录，而不是当前主线方案
3. 如需继续这条线，只研究：
   - `XPM`
   - 代码签名使能
   - HAP/HQF 部署后的 extension 装载条件
4. 按官方 `hosScrcpy` 的会话参数、socket 和生命周期收紧自研方案
5. 等到装载校验问题被实证解决后，再决定是否恢复完整 `scrcpy server` 风格实现

需要避免的误区：

- 不要把官方 `hosScrcpy` 路线误判成普通应用录屏 API
- 不要假设只要导出 `UiTestExtension_OnInit/OnRun` 就自动拥有虚拟屏和编码能力
- 不要假设自制 so 只要被编出来并打进 HAP，就一定能被 `uitest` 加载
- 不要一开始就完整重写 `scrcpy server`

更稳的步骤是：

- `load`
- `run`
- `hold`
- `socket`
- `virtual screen`
- `encoder surface`
- `video pipeline`

## 相关文档

- `UITest` 自定义扩展 POC：
  - [`docs/uitest-extension-poc.md`](/Users/magicdian/Documents/personal_project/hscrcpy/docs/uitest-extension-poc.md)
