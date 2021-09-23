# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.163.1/containers/rust/.devcontainer/base.Dockerfile

FROM mcr.microsoft.com/vscode/devcontainers/rust:0-1
ARG USERNAME=vscode
ARG USER_UID=1000
ARG USER_GID=$USER_UID

# Setup bash history
RUN SNIPPET="export PROMPT_COMMAND='history -a' && export HISTFILE=/commandhistory/.bash_history" \
    && mkdir /commandhistory \
    && touch /commandhistory/.bash_history \
    && chown -R $USERNAME /commandhistory \
    && echo $SNIPPET >> "/home/$USERNAME/.bashrc"

# Install postgres packages
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends libpq-dev postgresql-client mosquitto-clients
