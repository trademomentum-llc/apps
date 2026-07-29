REGRESSION_DIR := tests/regression

regression:
	@python3 scripts/jstar_regression.py $(REGRESSION_DIR)

regression-update:
	@python3 scripts/jstar_regression.py $(REGRESSION_DIR) --update
