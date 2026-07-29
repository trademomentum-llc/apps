REGRESSION_DIR := tests/regression
JASTERISH_COMPILER ?= $(shell [ -f target/debug/morphlex ] && echo target/debug/morphlex || echo morphlex)
export JASTERISH_COMPILER

regression:
	@python3 scripts/jstar_regression.py $(REGRESSION_DIR)

regression-update:
	@python3 scripts/jstar_regression.py $(REGRESSION_DIR) --update
