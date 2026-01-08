# CNI Gesprek Makefile
# Development and testing automation

.PHONY: help build test coverage coverage-report coverage-check clean

# Default target
help:
	@echo "CNI Gesprek Development Commands:"
	@echo "================================="
	@echo "build          - Build the project in release mode"
	@echo "test           - Run all tests"
	@echo "coverage       - Generate code coverage report"
	@echo "coverage-report- Open coverage report in browser"
	@echo "coverage-check - Check if coverage meets threshold"
	@echo "clean          - Clean build artifacts and coverage reports"

# Build the project
build:
	cargo build --release

# Run all tests
test:
	cargo test --all-features

# Generate code coverage report
coverage:
	@echo "Generating code coverage report..."
	@mkdir -p coverage
	cargo tarpaulin \
		--all-features \
		--workspace \
		--timeout 300 \
		--exclude-files "*/tests/*" "*/benches/*" "*/examples/*" \
		--ignore-panics \
		--out Html Xml Lcov \
		--output-dir ./coverage/
	@echo "Coverage report generated in ./coverage/"

# Open coverage report in browser
coverage-report: coverage
	@if command -v xdg-open >/dev/null 2>&1; then \
		xdg-open coverage/tarpaulin-report.html; \
	elif command -v open >/dev/null 2>&1; then \
		open coverage/tarpaulin-report.html; \
	else \
		echo "Coverage report available at: ./coverage/tarpaulin-report.html"; \
	fi

# Check coverage threshold (90% for core business logic)
coverage-check:
	@echo "Checking coverage threshold..."
	@mkdir -p coverage
	cargo tarpaulin \
		--all-features \
		--workspace \
		--timeout 300 \
		--exclude-files "*/tests/*" "*/benches/*" "*/examples/*" "*/src/testing/*" "*/src/bin/*" \
		--ignore-panics \
		--fail-under 90 \
		--output-dir ./coverage/
	@echo "✅ Coverage threshold met!"

# Clean build artifacts and coverage reports
clean:
	cargo clean
	rm -rf coverage/
	rm -f tarpaulin-report.html

# Install development dependencies
install-deps:
	@echo "Installing development dependencies..."
	@if ! command -v cargo-tarpaulin >/dev/null 2>&1; then \
		echo "Installing cargo-tarpaulin..."; \
		cargo install cargo-tarpaulin; \
	else \
		echo "cargo-tarpaulin already installed"; \
	fi
