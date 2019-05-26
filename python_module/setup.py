import toml
from setuptools import setup
from setuptools_rust import Binding, RustExtension

CARGO_MANIFEST_PATH = "../tkapp/Cargo.toml"

setup(
    name="mystcl",
    version="1.0.7",
    rust_extensions=[
        RustExtension("mystcl.mystcl", path=CARGO_MANIFEST_PATH, binding=Binding.PyO3)
    ],
    packages=["mystcl"],
    zip_safe=False,
)
