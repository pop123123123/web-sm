FROM ubuntu:22.04

ARG DEBIAN_FRONTEND=noninteractive

# Install wget, curl, gcc and gstreamer
RUN apt-get update && apt-get -y install wget curl gcc libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav libgstrtspserver-1.0-dev libges-1.0-dev


ARG MFA_HOME=/home/mfa
RUN useradd mfa && mkdir ${MFA_HOME} && chmod 777 ${MFA_HOME}
WORKDIR ${MFA_HOME}
USER mfa

# Install miniconda
RUN INSTALLER="Miniconda3-py310_23.1.0-1-Linux-x86_64.sh" && wget "https://repo.anaconda.com/miniconda/${INSTALLER}" && chmod +x ./${INSTALLER} && ./${INSTALLER} -b -p ./miniconda3 && rm ./${INSTALLER}
ENV PATH ${MFA_HOME}/miniconda3/bin:$PATH

# Install montreal forced aligner
RUN conda config --add channels conda-forge && conda install -y montreal-forced-aligner && mfa model download dictionary french_mfa && mfa model download acoustic french_mfa && mfa model download g2p french_mfa

# # Install rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH ${MFA_HOME}/.cargo/bin:$PATH

# Project folder
ARG WEBSM=${MFA_HOME}/websm
COPY --chown=mfa . ${WEBSM}/
WORKDIR ${WEBSM}

# Install sentence-mixing
RUN pip3 install -r sm-interface/requirements.txt

# Install backend
RUN cargo build --release
