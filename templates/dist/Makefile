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
	@cp -r .${PROVIDER}/* "$(TARGET)/"
	@echo "Installed to $(TARGET)/"
