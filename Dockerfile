FROM ubuntu:22.04

# Install miniconda
RUN apt-get update && apt-get -y install wget && INSTALLER="Miniconda3-py310_23.1.0-1-Linux-x86_64.sh" && wget "https://repo.anaconda.com/miniconda/${INSTALLER}" && chmod +x ./${INSTALLER} && ./${INSTALLER} -b -p /miniconda3 && rm ./${INSTALLER}
ENV PATH /miniconda3/bin:$PATH

# Install montreal forced aligner
RUN conda config --add channels conda-forge && conda install -y montreal-forced-aligner && mfa model download dictionary french_mfa && mfa model download acoustic french_mfa && mfa model download g2p french_mfa
