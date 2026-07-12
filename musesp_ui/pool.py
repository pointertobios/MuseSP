"""多线程计算池。numpy 操作释放 GIL，线程并行有效。"""

import atexit
import os
from concurrent.futures import Future, ThreadPoolExecutor

_pool: ThreadPoolExecutor | None = None


def get_pool() -> ThreadPoolExecutor:
    """获取全局线程池（惰性初始化）。"""
    global _pool
    if _pool is None:
        workers = min(os.cpu_count() or 4, 8)
        _pool = ThreadPoolExecutor(max_workers=workers)
    return _pool


def shutdown() -> None:
    """关闭线程池。"""
    global _pool
    if _pool is not None:
        _pool.shutdown(wait=True)
        _pool = None


atexit.register(shutdown)
