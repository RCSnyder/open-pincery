# /// script
# requires-python = ">=3.10"
# dependencies = ["pymupdf"]
# ///

import fitz
import sys

doc = fitz.open("continuous_agent_architecture.pdf")
for page in doc:
    print(page.get_text())
