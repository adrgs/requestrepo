import sys
import os
from pathlib import Path

# Add backend directory to Python path
project_root = Path(__file__).parent.parent.parent
backend_dir = project_root / "backend"

# Add backend directory to Python path
if backend_dir.exists():
    sys.path.insert(0, str(backend_dir))
