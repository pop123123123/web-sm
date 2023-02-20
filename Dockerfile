FROM ubuntu:22.04

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get -y install wget curl gcc

# Install miniconda
RUN INSTALLER="Miniconda3-py310_23.1.0-1-Linux-x86_64.sh" && wget "https://repo.anaconda.com/miniconda/${INSTALLER}" && chmod +x ./${INSTALLER} && ./${INSTALLER} -b -p /miniconda3 && rm ./${INSTALLER}
ENV PATH /miniconda3/bin:$PATH

# Install montreal forced aligner
RUN conda config --add channels conda-forge && conda install -y montreal-forced-aligner && mfa model download dictionary french_mfa && mfa model download acoustic french_mfa && mfa model download g2p french_mfa

# Install rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH /root/.cargo/bin:$PATH

# Install gstreamer
RUN apt-get install -y libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav libgstrtspserver-1.0-dev libges-1.0-dev

# Project folder
ARG WEBSM=/websm
WORKDIR ${WEBSM}
COPY . ${WEBSM}/

# Install sentence-mixing
RUN pip3 install -r sm-interface/requirements.txt

# Install backend
RUN cargo build --release
