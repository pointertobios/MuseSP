"""只读配置模块。

配置文件为 TOML 格式，通过命令行 -c 参数指定路径，
未指定时默认使用 ./config.example.toml。

用法:
    from musesp_config.config import config
    if config.debug.ui.component_border:
        ...
"""

import sys
import tomllib
from pathlib import Path


class _Config:
    """只读配置对象，支持嵌套属性访问。"""

    def __init__(self, data: dict) -> None:
        for key, value in data.items():
            if isinstance(value, dict):
                object.__setattr__(self, key, _Config(value))
            else:
                object.__setattr__(self, key, value)
        object.__setattr__(self, "_frozen", True)

    def __setattr__(self, name: str, value: object) -> None:
        if getattr(self, "_frozen", False):
            raise AttributeError(f"config 为只读，不可设置 '{name}'")
        object.__setattr__(self, name, value)

    def __repr__(self) -> str:
        items = {k: v for k, v in self.__dict__.items() if not k.startswith("_")}
        return f"_Config({items})"


def _resolve_config_path() -> Path:
    """从命令行 -c 参数解析配置文件路径，未指定则使用默认值。"""
    argv = sys.argv[1:]
    for i, arg in enumerate(argv):
        if arg == "-c" and i + 1 < len(argv):
            return Path(argv[i + 1])
    return Path("config.example.toml")


def _load_config() -> _Config:
    path = _resolve_config_path()
    if not path.exists():
        raise FileNotFoundError(f"配置文件不存在: {path}")
    with open(path, "rb") as f:
        data = tomllib.load(f)
    return _Config(data)


config = _load_config()
