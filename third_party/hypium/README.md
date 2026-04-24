# Hypium / hosScrcpy 官方 so 归档

## 目标

这个目录用于归档 HarmonyOS 官方测试框架相关 so，方便后续做：

- 不同版本兼容性对比
- 不同 ABI 产物对比
- `hosScrcpy` 与 `xdevice-devicetest` 二进制行为比对

## 目录约定

### `hosScrcpy/<version>/libscrcpy`

来源：

- `DevecoTesting-Hypium/lib/hosScrcpy-*.jar`
- `DevecoTesting-Hypium/lib/DevecoTesting-Hypium-*.jar`

内容：

- `libscrcpy_server*.z.so`
- `libscrcpy_server_unix_*.z.so`
- `libscrcpy_server_emulator.z.so`

说明：

- `unix` 版本主要对应 `localabstract:*` socket 模式
- `emulator` 版本对应模拟器环境

### `hosScrcpy/<version>/uitest_agent`

来源：

- `hosScrcpy-*.jar`

内容：

- `uitest_agent_*.so`
- 包含 arm64 与 x86 相关版本

### `xdevice-devicetest/<version>/recorder`

来源：

- `xdevice_devicetest-*.whl`

内容：

- `libscrcpy_server*.z.so`

说明：

- 这条线更接近官方 Python 控制层 `record_agent.py`

### `xdevice-devicetest/<version>/uitest_agent`

来源：

- `xdevice_devicetest-*.whl`

内容：

- `uitest_agent_v*.so`

## 当前已归档版本

- `hosScrcpy/6.1.0.210`
- `xdevice-devicetest/6.1.0.210`

