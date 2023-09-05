#!/bin/sh

cd cert
\rm *.pem
mkcert alice.example
mkcert bob.example
mkcert charlie.example
