# Project Guidelines

## Python 包结构

- 不使用 `__init__.py`，采用现代 flat module 方式，每个子包直接以 `.py` 文件暴露接口
- 模块目录名使用下划线（`musesp_ui`、`musesp_asset`、`musesp_gameplay`），不得使用连字符

## 包声明与构建

- 所有子包通过 `pyproject.toml` 的 `[tool.setuptools.packages.find]` 声明，使用 `include = ["musesp*"]` 模式
- 使用 `uv` 作为包管理器
- 修改 `pyproject.toml` 或目录结构后，需执行 `uv sync` 使子包可被导入
