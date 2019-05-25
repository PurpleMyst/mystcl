import toml
from setuptools import setup
from setuptools_rust import Binding, RustExtension

with open("Cargo.toml") as f:
    cargo_manifest = toml.load(f)
    version = cargo_manifest["package"]["version"]

setup(
    name="mystcl",
    version=version,
    rust_extensions=[RustExtension("mystcl.mystcl", binding=Binding.PyO3)],
    packages=["mystcl"],
    zip_safe=False,
)
