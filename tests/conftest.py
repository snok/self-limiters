import logging

import pytest as pytest
from maturin import import_hook


@pytest.fixture(autouse=True, scope='session')
def _install_package():
    import_hook.install(bindings='pyo3')


@pytest.fixture(autouse=True, scope='session')
def _setup_logging():
    log_format = '[%(asctime)s] [%(levelname)s] - %(message)s'
    logging.basicConfig(level='DEBUG', format=log_format)
