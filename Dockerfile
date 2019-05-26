FROM quay.io/pypa/manylinux2010_x86_64

RUN yum install -y clang xorg-x11-server-Xvfb xorg-x11-fonts-*

RUN git clone https://github.com/tcltk/tcl /tcl
WORKDIR /tcl/unix
RUN git checkout core-8-6-9
RUN ./configure
RUN make -j2
RUN make install

RUN git clone https://github.com/tcltk/tk /tk
WORKDIR /tk/unix
RUN git checkout core-8-6-9
RUN ./configure
RUN make -j2
RUN make install

RUN curl https://sh.rustup.rs -f | sh -s -- --default-toolchain nightly -y
ENV PATH="/root/.cargo/bin:${PATH}"

CMD /app/scripts/build_wheels
