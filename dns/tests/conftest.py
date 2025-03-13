import sys
import os
from pathlib import Path

# Add dns directory to Python path
project_root = Path(__file__).parent.parent.parent
dns_dir = project_root / "dns"

# Add dns directory to Python path
if dns_dir.exists():
    sys.path.insert(0, str(dns_dir)) 