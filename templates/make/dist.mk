# Install to user or workspace scope
#   make install                  # user scope
#   make install SCOPE=workspace  # project scope

SCOPE ?= user

ifeq ($(SCOPE),workspace)
	TARGET := .${PROVIDER}
else
	TARGET := $(HOME)/.${PROVIDER}
endif

.PHONY: install

install:
	@mkdir -p "$(TARGET)"
	@SRC="$$(cd .${PROVIDER} && pwd)"; \
	 DST="$$(cd "$(TARGET)" && pwd)"; \
	 if [ "$$SRC" = "$$DST" ]; then \
	     echo "Already in $(TARGET)/ — nothing to do"; \
	 else \
	     cp -R .${PROVIDER}/. "$(TARGET)/" && echo "Installed to $(TARGET)/"; \
	 fi
