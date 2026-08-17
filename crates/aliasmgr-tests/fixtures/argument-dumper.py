import json
import sys

print(json.dumps(sys.argv[1:], ensure_ascii=False))
