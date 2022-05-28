import json
from .common import *

proj_path = BASE_DIR
secrets_path = proj_path.joinpath('common/data/secrets.json')
secrets = None

AUTH_KEYSTORE['persist_secret_handler']['init_kwargs']['filepath'] = './tmp/cache/test/jwks/privkey/current.json'
AUTH_KEYSTORE['persist_pubkey_handler']['init_kwargs']['filepath'] = './tmp/cache/test/jwks/pubkey/current.json'
AUTH_KEYSTORE['persist_secret_handler']['init_kwargs']['flush_threshold'] = 4
AUTH_KEYSTORE['persist_pubkey_handler']['init_kwargs']['flush_threshold'] = 4

with open(secrets_path, 'r') as f:
    secrets = json.load(f)
    secrets = secrets['backend_apps']['databases']['test_site_dba']

# Django test only uses `default` alias , which does NOT allow users to switch
# between different database credentials
DATABASES['default'].update(secrets)
DATABASES['default']['NAME'] = DATABASES['default']['TEST']['NAME']
## does NOT work for testing
##DATABASES['usermgt_service'].update(secrets)
DATABASE_ROUTERS.clear()
render_logging_handler_localfs('tmp/log/test')

