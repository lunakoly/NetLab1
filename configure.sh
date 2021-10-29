#!/bin/sh

HERE=$(dirname "$0")

ln -s ../target ${HERE}/client/target
ln -s ../target ${HERE}/server/target
