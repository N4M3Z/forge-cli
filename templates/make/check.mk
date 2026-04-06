# Module structure check — include from project Makefile
# Usage: include templates/mk/check.mk

.PHONY: check

check:
	@test -f module.yaml      && echo "  ok module.yaml"      || echo "  MISSING module.yaml"
	@test -f defaults.yaml    && echo "  ok defaults.yaml"    || echo "  MISSING defaults.yaml"
	@test -f README.md        && echo "  ok README.md"        || echo "  MISSING README.md"
	@test -f LICENSE          && echo "  ok LICENSE"          || echo "  MISSING LICENSE"
	@test -f INSTALL.md       && echo "  ok INSTALL.md"       || echo "  MISSING INSTALL.md (optional)"
	@test -f CONTRIBUTING.md  && echo "  ok CONTRIBUTING.md"  || echo "  MISSING CONTRIBUTING.md (optional)"
	@test -f CODEOWNERS       && echo "  ok CODEOWNERS"       || echo "  MISSING CODEOWNERS (optional)"
	@test -f CHANGELOG.md     && echo "  ok CHANGELOG.md"     || echo "  MISSING CHANGELOG.md (optional)"
	@test -f .gitattributes   && echo "  ok .gitattributes"   || echo "  MISSING .gitattributes (optional)"
