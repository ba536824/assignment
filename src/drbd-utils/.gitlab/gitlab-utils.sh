# Helper functions for CI

# Prepare directories:
# * download - expected to be cached
# * bin - executables on path
# Also fetch rq
drbd_prepare_tools() {
	mkdir -p download bin
	PATH="$(readlink -f bin):$PATH"

	[ -e download/rq ] || curl -sSfL https://github.com/dflemstr/rq/releases/download/v1.0.2/rq-v1.0.2-x86_64-unknown-linux-gnu.tar.gz | tar -C download -xvzf -
	ln -s ../download/rq bin/rq
}

# Fetch lbbuildctl; do not cache it because we always want the latest version
drbd_fetch_lbbuildctl() {
	curl -sSL $LINBIT_REGISTRY_URL/repository/lbbuild/lbbuildctl-latest -o bin/lbbuildctl
	chmod +x bin/lbbuildctl
}
