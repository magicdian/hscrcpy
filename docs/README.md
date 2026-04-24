# Docs 目录说明

## 目标

`docs/` 下的文档分成两类：

- 正式开发文档
- 本地逆向记录

后续需要上传到 git 服务器的内容，应优先放在 `docs/` 根目录或 `docs/architecture/` 中；仅供本地分析的内容，单独归档，不纳入正式文档目录约定。

## 目录约定

### 正式开发文档

放在这些位置：

- `docs/*.md`
- `docs/architecture/*.md`

当前包括：

- [`docs/harmony-hos-scrcpy-debug-guide.md`](/Users/magicdian/Documents/personal_project/hscrcpy/docs/harmony-hos-scrcpy-debug-guide.md)
- [`docs/uitest-extension-poc.md`](/Users/magicdian/Documents/personal_project/hscrcpy/docs/uitest-extension-poc.md)
- [`docs/architecture/host-device-mvp-contract.md`](/Users/magicdian/Documents/personal_project/hscrcpy/docs/architecture/host-device-mvp-contract.md)

这些文档的要求是：

- 面向正向开发
- 只保留稳定、可执行、可复现的信息
- 避免混入针对某个第三方产品的逆向叙事

### 本地逆向记录

这个目录用于：

- 第三方产品逆向分析
- 样本文件记录
- 本地临时调查笔记

这类内容默认不作为正式开发文档的一部分。

## 当前建议

后续整理文档时，按下面的边界处理：

- 只谈官方 `hosScrcpy / UITest Agent / xdevice-devicetest` 的使用、调试、接口和实验方法
  - 放 `docs/` 根目录
- 只谈第三方产品的实现分析、来源判断、样本对比
  - 单独归档，不作为正式开发文档入口
