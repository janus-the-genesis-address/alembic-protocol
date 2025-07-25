#!/usr/bin/env bash
set -ex

[[ $(uname) = Linux ]] || exit 1
[[ $USER = root ]] || exit 1

if grep -q Alembic /etc/passwd ; then
  echo "User Alembic already exists"
else
  adduser Alembic --gecos "" --disabled-password --quiet
  adduser Alembic sudo
  adduser Alembic adm
  echo "Alembic ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers
  id Alembic

  [[ -r /Alembic-scratch/id_ecdsa ]] || exit 1
  [[ -r /Alembic-scratch/id_ecdsa.pub ]] || exit 1

  sudo -u Alembic bash -c "
    echo 'PATH=\"/home/Alembic/.cargo/bin:$PATH\"' > /home/Alembic/.profile
    mkdir -p /home/Alembic/.ssh/
    cd /home/Alembic/.ssh/
    cp /Alembic-scratch/id_ecdsa.pub authorized_keys
    umask 377
    cp /Alembic-scratch/id_ecdsa id_ecdsa
    echo \"
      Host *
      BatchMode yes
      IdentityFile ~/.ssh/id_ecdsa
      StrictHostKeyChecking no
    \" > config
  "
fi
