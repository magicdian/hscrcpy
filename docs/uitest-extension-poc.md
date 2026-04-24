# UITest 自定义扩展 POC 说明

## 目标

这份 POC 不是为了直接重写官方 `scrcpy server`，而是先验证一个更基础的问题：

- 我们自己编译的 `.so`
- 只导出 `UiTestExtension_OnInit` / `UiTestExtension_OnRun`
- 不依赖第三方私有代码

是否也能被 HarmonyOS 的 `/system/bin/uitest` 通过 `--extension-name` 正常拉起。

如果这个最小实验成立，后续再继续往里叠：

- 参数解析
- 长连接服务
- `uitest_socket` 或自定义 socket
- 虚拟屏
- 编码器

## 当前实现

代码位置：

- [`sources/hscrcpy_server/entry/src/main/cpp/poc/uitest_extension_poc.cpp`](/Users/magicdian/Documents/personal_project/hscrcpy/sources/hscrcpy_server/entry/src/main/cpp/poc/uitest_extension_poc.cpp)

构建目标：

- `hscrcpy_uitest_poc.so`

当前行为非常简单：

1. `UiTestExtension_OnInit`
   - 记录 `token`
   - 记录 `argc/argv`
   - 解析几个测试参数：
     - `--hold-seconds`
     - `--heartbeat-ms`
     - `--process-name`
     - `--probe-screen-capture-auth`
     - `--probe-screen-capture-each-heartbeat`
2. `UiTestExtension_OnRun`
   - 可选设置进程名
   - 打印当前 `pid/uid/gid`
   - 在指定时间内持续打印 heartbeat
   - 可选调用 `OH_AVScreenCapture` 探针
   - 到时后退出

关键日志统一走：

- `tag=hscrcpyDiag`
- `subsystem=poc/uitest_extension`

当前 POC 重点回答两个问题：

1. `uitest` 是否允许加载我们自己编译、非华为签名的 `.so`
2. 如果允许加载，当前 extension 进程里调用 `OH_AVScreenCapture_StartScreenCapture` 会得到什么结果

## 实验记录

本轮已完成一次完整对照实验，结论已经比较明确。

### 实验材料

自制 POC：

- [`sources/hscrcpy_server/entry/build/default/intermediates/cmake/default/obj/arm64-v8a/hscrcpy_uitest_poc.so`](/Users/magicdian/Documents/personal_project/hscrcpy/sources/hscrcpy_server/entry/build/default/intermediates/cmake/default/obj/arm64-v8a/hscrcpy_uitest_poc.so)

官方对照：

- `uitest_agent_1.2.3.so`

### 实验步骤

1. 将自制 `hscrcpy_uitest_poc.so` 推送到：
   - `/data/local/tmp/hscrcpy_uitest_poc.so`
2. 通过：
   - `uitest start-daemon singleness --extension-name hscrcpy_uitest_poc.so ...`
   尝试拉起
3. 同时推送官方：
   - `/data/local/tmp/uitest_agent_1.2.3.so`
4. 通过：
   - `uitest start-daemon singleness --extension-name uitest_agent_1.2.3.so`
   进行对照
5. 两边都额外执行过：
   - `chmod 755`

### 实验结果

自制 `hscrcpy_uitest_poc.so`：

- `UiTestExtension_OnInit`
- `UiTestExtension_OnRun`

两个入口函数都在动态符号表中，说明 so 结构本身没有缺入口。

但设备日志显示：

- `Xpm check failed for /data/local/tmp/hscrcpy_uitest_poc.so`
- `dlopen_impl load library header failed`
- `Dlopen ... Permission denied`

也就是说：

- 代码根本没有执行到 `UiTestExtension_OnInit`
- 失败发生在 `uitest` 的装载校验阶段

官方 `uitest_agent_1.2.3.so`：

- 同样放在 `/data/local/tmp`
- 同样执行了 `chmod 755`
- 可以正常启动，并能看到：
  - `Welcome to devicetest agent so!`
  - `UiTestExtension_OnInit done, uitestVersion=..., extensionVersion=1.2.3`

### 额外核对

还额外确认了下面几件事：

1. `chmod +x` 不是问题核心
   - 官方 so 与自制 so 都加了执行位
   - 只有官方 so 可以加载
2. 自制 so 已经被打进 signed HAP
   - `entry-default-signed.hap` 中包含：
     - `libs/arm64-v8a/hscrcpy_uitest_poc.so`
3. 从 signed HAP 解出的最终 so 与：
   - `intermediates/stripped_native_libs/.../hscrcpy_uitest_poc.so`
   逐字节一致
4. 但这个最终 so 本体里仍然看不到官方 so 常见的受信任指纹：
   - `Huawei CBG ...`
   - `Software Signing Service CA`

### 当前结论

基于本轮实验，目前最稳妥的结论是：

- 当前设备上的 `uitest` 不接受这份自制 extension so
- blocker 不在：
  - 文件路径
  - 文件执行位
  - 导出符号
  - 是否打进 HAP
- blocker 更像是：
  - `XPM`
  - 代码签名使能
  - 或 `uitest` 对受信任 extension so 的额外装载校验

因此，现阶段不应再把“自己写一个 so，直接给 `uitest --extension-name` 加载”视为主路径。

### 当前未能回答的问题

由于自制 so 根本没有被成功加载，所以本轮还无法回答：

- 在自制 extension 进程里，`OH_AVScreenCapture` 是否可用
- 是否会弹授权框
- 是否能进一步触达虚拟屏或编码器相关能力

## 手动测试步骤

### 1. 编译产物

先正常编译 `sources/hscrcpy_server` 的 native 产物，然后找到：

- `hscrcpy_uitest_poc.so`

如果不确定输出目录，可以在工程根目录执行：

```bash
find sources/hscrcpy_server -name 'hscrcpy_uitest_poc.so'
```

### 2. 推送到设备

```bash
DEVICE="$(hdc list targets | awk 'NR==1{print $1}')"
SO_PATH="$(find sources/hscrcpy_server -name 'hscrcpy_uitest_poc.so' | head -1)"

hdc -t "$DEVICE" file send "$SO_PATH" /data/local/tmp/hscrcpy_uitest_poc.so
```

### 3. 启动扩展

```bash
DEVICE="$(hdc list targets | awk 'NR==1{print $1}')"
hdc -t "$DEVICE" shell \
  'uitest start-daemon singleness \
    --extension-name hscrcpy_uitest_poc.so \
    --process-name hscrcpy_poc \
    --hold-seconds 20 \
    --heartbeat-ms 1000'
```

如果要顺便验证当前 extension 里是否具备 `AVScreenCapture` 录屏能力，改用：

```bash
DEVICE="$(hdc list targets | awk 'NR==1{print $1}')"
hdc -t "$DEVICE" shell \
  'uitest start-daemon singleness \
    --extension-name hscrcpy_uitest_poc.so \
    --process-name hscrcpy_poc \
    --hold-seconds 20 \
    --heartbeat-ms 1000 \
    --probe-screen-capture-auth'
```

### 4. 观察日志

```bash
DEVICE="$(hdc list targets | awk 'NR==1{print $1}')"
hdc -t "$DEVICE" shell \
  'hilog -x | grep -E "hscrcpyDiag|poc/uitest_extension|uitest"'
```

### 5. 观察进程

```bash
DEVICE="$(hdc list targets | awk 'NR==1{print $1}')"
hdc -t "$DEVICE" shell 'ps -ef | grep -E "[h]scrcpy_poc|[u]itest start-daemon"'
```

## 预期结果

如果 POC 可以被 `uitest` 正常加载，应该至少看到：

1. `hilog` 中出现：
   - `subsystem=poc/uitest_extension operation=on_init`
   - `subsystem=poc/uitest_extension operation=on_run_start`
   - `subsystem=poc/uitest_extension operation=runtime_identity`
   - 多条 `heartbeat`
2. `ps` 中出现自定义进程名：
   - `hscrcpy_poc`
3. 到达 `--hold-seconds` 后：
   - `on_run_finish`
   - 进程退出

如果加了 `--probe-screen-capture-auth`，还应出现：

- `subsystem=poc/uitest_extension operation=screen_capture_probe`

其中 `authorization_state` 目前会收敛到：

- `granted`
- `needs_user_action`
- `unsupported`

## 这次 POC 能回答什么

如果实验成功，可以先回答下面这个问题：

- 我们自己写一个最小 `scrcpy_server.so` 风格扩展，有没有机会被 `uitest` 拉起？

但截至当前实验结果，这个问题的答案应暂时改成：

- 目前不可行
- 至少在当前设备与当前部署方式下，`uitest` 不接受这份自制 so 的装载

但这还不能直接证明下面这些更难的能力也已经可用：

- 创建虚拟屏
- 获取安全豁免
- 调系统编码器
- 暴露 `scrcpy_grpc_socket`
- 复刻官方 `xdevice_scrcpy` 风格会话链路

同时，这次 POC 原计划补充回答：

- 当前 `uitest extension` 进程里，`AVScreenCapture` 是直接可用、需要用户授权，还是根本不可用

但由于 extension 未成功启动，这个问题目前仍未完成验证。

## 下一步建议

基于当前实验结果，下一步建议调整为：

1. 暂停继续扩大自制 `uitest extension so` 的功能面
2. 如果还要继续这条线，只研究：
   - `XPM`
   - 代码签名使能
   - HAP/HQF 部署后的 extension 装载条件
3. 主开发路线回到：
   - 公开应用侧 `AVScreenCapture`
   - 官方 `hosScrcpy / uitest_agent` 行为参考
