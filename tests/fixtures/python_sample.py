# Test fixture for Python syntax highlighting
# Should test: strings, keywords, comments

# String literals
greeting = "Hello, world!"
multiline = """This is a
multiline string"""
raw_string = r"Raw string with \n no escape"
f_string = f"Format string with {greeting}"

# Keywords - control flow
if True:
    print("if keyword")
elif False:
    print("elif keyword")
else:
    print("else keyword")

# Keywords - loops
for i in range(10):
    if i == 5:
        break
    if i == 3:
        continue
    pass

while False:
    print("never executed")

# Keywords - function
def greet(name):
    return f"Hello, {name}!"

async def fetch_data():
    await some_coroutine()
    return "data"

# Keywords - class
class Person:
    def __init__(self, name):
        self.name = name

    @staticmethod
    def create(name):
        return Person(name)

    @classmethod
    def from_dict(cls, data):
        return cls(data['name'])

# Keywords - exception handling
try:
    raise ValueError("error")
except ValueError as e:
    print(f"caught: {e}")
finally:
    print("cleanup")

# Keywords - context manager
with open("file.txt") as f:
    content = f.read()

# Keywords - import
import os
from pathlib import Path
import sys as system

# Keywords - other
assert True
del variable
global global_var
nonlocal outer_var
lambda x: x + 1
yield 42
