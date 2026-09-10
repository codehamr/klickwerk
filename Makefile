.PHONY: all dev test check browser-setup clean

all:
	npm run desktop:build

dev:
	npm run dev

check:
	npm run check
	npm run test:core

test: check
	npm test

browser-setup:
	npx playwright install --with-deps chromium

# Keep config.cfg and the last working EXE. Clear only regenerable UI files.
clean:
	node scripts/clean.mjs
