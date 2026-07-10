---
name: create-page
description: "为 MuseSP 项目编写 Page 子类。使用 ask-questions 工具逐一询问页面行为（遮挡、组件、导航），再生成代码。Use when: 创建新页面, 写 Page, 加页面, create page, new page, add page."
argument-hint: "描述这个页面要做什么"
user-invocable: true
---

# 编写 Page

基于 `musesp_ui.router.Page` 接口，引导用户编写一个新的 Page 子类。

## 流程

加载本 skill 后，必须使用 `vscode_askQuestions` 工具分**两轮**向用户提问。第一轮确定基本信息和遮挡/事件行为；第二轮确定组件和导航逻辑。

### 第一轮提问

| 问题                           | 参考选项                                                         |
| ------------------------------ | ---------------------------------------------------------------- |
| **页面名称**（Page 类名）      | `HomePage`、`SettingsPage`、`DialogPage`、`MenuPage`…            |
| **所属模块**（放在哪个子包下） | `musesp/home.py`、`musesp_editor/settings.py`…                   |
| **`full_shadow_promise()`**    | `True`（完全遮挡下层，跳过绘制下层）或 `False`（默认，下层可见） |
| **`hide_last()` 行为**         | 无操作（默认）/ 保存截图 / 模糊上一页 / …                        |

### 第二轮提问

| 问题                     | 参考选项                             |
| ------------------------ | ------------------------------------ |
| **需要的组件**           | 无 / `Label` / `Button` / 多个组合…  |
| **`build()` 中做什么**   | 创建组件、`add_component`、加载资源… |
| **`destroy()` 中做什么** | 无操作（默认）/ 释放资源…            |
| **是否 `push` 子页面**   | 无 / 点击按钮 push 某页面 / …        |
| **`pop` 返回值**         | `None` / 字符串 / 选中项…            |

## 生成物

根据用户回答，生成：

1. Page 子类 Python 文件，放在指定模块路径下
2. 带完整 `build`/`destroy` 逻辑
3. 组件通过 `add_component` 注册到页面根组件
4. 如有子页面导航，在对应组件 `bind_on_*` 中调用 `router.push`/`router.pop`

## 参考

Page 接口定义见 [router.py](../../musesp_ui/router.py)，可用组件见 [components.py](../../musesp_ui/components.py)。

### Page 生命周期（调用顺序）

```
build() → [on_hide()] → [on_activate()] → … → destroy()
           ↑ push 时        ↑ 子页 pop 时          ↑ pop 本页时
```

### 组件绑定示例

```python
btn.bind_on_mouse_click(lambda e: router.pop("confirmed"))
```
