from setuptools import setup
from setuptools_rust import Binding, RustExtension

setup(
    name="mystcl",
    version="1.0.0",
    rust_extensions=[RustExtension("mystcl.mystcl", binding=Binding.PyO3)],
    packages=["mystcl"],
    zip_safe=False,
)
