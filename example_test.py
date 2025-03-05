def foo() -> int:
    return 1


def bar() -> int:
    return foo() + 1


def baz() -> int:
    return bar() + 1


def test_foo():
    assert foo() == 1
