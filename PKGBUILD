pkgname=mu
pkgver=0.1
pkgrel=1
pkgdesc="Advanced Unix-style Memory Card System"
arch=("any")
url="https://github.com/ticki/mu"
license=('MIT')
depends=('zathura')
makedepends=('rust')
source=("git+https://github.com/ticki/$pkgname.git")
sha1sums=('SKIP')

pkgver() {
    (git describe --long --tags || echo "$pkgver") | sed 's/^v//;s/\([^-]*-g\)/r\1/;s/-/./g'
}

build() {
    cargo build --release
    cd ../mkmu
    cargo build --release
}

package() {
    cd ..
    usrdir="$pkgdir/usr"
    mkdir -p $usrdir
    cargo install --path . --root "$usrdir"
    cargo install --path mkmu --root "$usrdir"
    rm -f $usrdir/.crates.toml
}
