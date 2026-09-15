from datetime import datetime


def greet(name: str) -> str:
    return f"Hello, {name}!"


print(greet("minamilist"))
print(f"Python ran this file at {datetime.now():%H:%M:%S}")
