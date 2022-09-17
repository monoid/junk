import pyutf8str
import pytest

SAMPLES = ["", "test", "tést", "t\x7Fst", "t\x80st" "тест", "t\uFFFFst", "test🤪"]


@pytest.mark.parametrize("uni", SAMPLES)
def test_eq(uni):
    assert uni == pyutf8str.Utf8Str(uni)


@pytest.mark.parametrize("uni", SAMPLES)
def test_str(uni):
    assert uni == str(pyutf8str.Utf8Str(uni))


@pytest.mark.parametrize("uni", SAMPLES)
def test_hash(uni):
    assert hash(uni) == hash(pyutf8str.Utf8Str(uni))


@pytest.mark.parametrize("uni", SAMPLES)
def test_repr(uni):
    assert repr(uni) == repr(pyutf8str.Utf8Str(uni))


def test_eq2():
    assert not (pyutf8str.Utf8Str("test") == 42)


def test_eq_bin():
    assert pyutf8str.Utf8Str("test") != b"test"


def test_bool():
    assert not bool(pyutf8str.Utf8Str(""))
    assert bool(pyutf8str.Utf8Str("test"))
