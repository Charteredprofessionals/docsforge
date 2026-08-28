import os, sys
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'src'))
sys.path.insert(0, os.path.dirname(__file__))

collect_ignore = [
    "test_log.txt",
    "test_log_final.txt",
    "src-tauri/test_log.txt",
    "src-tauri/test_log_final.txt",
]
