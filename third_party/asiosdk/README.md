# Steinberg ASIO SDK 2.3.4 (the parts the standalones build with)

The host side of the ASIO SDK, `common/` and `host/` without the sample, copied unchanged from
Steinberg's SDK 2.3.4 (`changes.txt`). `scripts/build-standalones.sh` points `CPAL_ASIO_DIR` here on
Windows, so the standalone programs offer ASIO beside WASAPI and JACK (cpal's `asio-sys` builds these files).

**Licence.** Steinberg licenses the SDK from 2.3.4 under its proprietary ASIO license or, alternatively,
under the GNU General Public License version 3 (`LICENSE.txt`, `README.md` of the SDK). HØJT DSP takes
the GPLv3 alternative, the licence of this repository (`LICENSE`), and passes the SDK on under it.
The proprietary agreement is not used. "ASIO" is a trademark of Steinberg Media Technologies GmbH.

To update: replace `common/` and `host/` (no `sample/`) and the two licence files with the new SDK's,
and keep the version here.
