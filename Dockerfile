FROM quay.io/pypa/manylinux2010_x86_64

RUN yum install -y clang xorg-x11-server-Xvfb

RUN git clone https://github.com/tcltk/tcl /tcl
WORKDIR /tcl/unix
RUN git checkout core-8-6-9
RUN ./configure
RUN make
RUN make install

RUN git clone https://github.com/tcltk/tk /tk
WORKDIR /tk/unix
RUN git checkout core-8-6-9
RUN ./configure
RUN make
RUN make install

RUN curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh -s -- --default-toolchain nightly -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app
COPY . .

RUN ./build_wheels

RUN /opt/python/cp37-cp37m/bin/python -m pip install twine
