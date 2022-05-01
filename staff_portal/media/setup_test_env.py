import sys
import json
import subprocess
import pdb

from MySQLdb import _mysql
from common.util.python import import_module_string

TEST_DB_MIGRATION_ALIAS = 'db_test_migration'

_is_test_migration_found = lambda cfg: cfg.get('alias') == TEST_DB_MIGRATION_ALIAS

class AbstractTestDatabase:
    def start(self, setting_path:str, liquibase_path:str):
        f = None
        renew_required = []
        cfg_root = {}
        with open(setting_path, 'r') as f:
            cfg_root = json.load(f)
            test_cfg = list(filter(_is_test_migration_found, cfg_root['databases']))
            if any(test_cfg):
                test_cfg = test_cfg[0]
                test_cfg['liquibase_path'] = liquibase_path
                credential = self.load_db_credential(filepath=test_cfg['credential']['filepath'],
                        hierarchy=test_cfg['credential']['hierarchy'])
                test_cfg['credential'] = credential
                self.setup_test_db(cfg=test_cfg)
            else:
                err_msg = 'the alias `%s` must be present in database configuration file' \
                        % TEST_DB_MIGRATION_ALIAS
                raise ValueError(err_msg)

    def load_db_credential(self, filepath:str, hierarchy):
        target = None
        with open(filepath , 'r') as f:
            target = json.load(f)
            for token in hierarchy:
                target = target[token]
        if target:
            target = {'host' : target['HOST'],  'port' : int(target['PORT']),
                'user' : target['USER'],  'passwd' : target['PASSWORD'] }
        return target

    def setup_test_db(self, cfg):
        raise NotImplementedError()

    def _create_drop_db(self, cfg, sql):
        credential = cfg['credential']
        credential.update({'connect_timeout':30})
        db_conn = None
        try:
            db_conn = _mysql.connect(**credential)
            db_conn.query(sql)
        finally:
            if db_conn:
                db_conn.close()

    def db_schema_cmd(self, cfg):
        credential = cfg['credential']
        return ['%s/liquibase' % cfg['liquibase_path'],
                '--defaults-file=./media/liquibase.properties',
                '--changeLogFile=./media/migration/changelog_media.xml',
                '--url=jdbc:mariadb://%s:%s/%s'
                    % (credential['host'], credential['port'], cfg['db_name']),
                '--username=%s' % credential['user'],
                '--password=%s' % credential['passwd'],
                '--log-level=info']
## end of AbstractTestDatabase


class StartTestDatabase(AbstractTestDatabase):
    def db_schema_cmd(self, cfg):
        out = super().db_schema_cmd(cfg)
        out.append('update')
        return out

    def setup_test_db(self, cfg):
        sql = 'CREATE DATABASE `%s` DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;' \
                % cfg['db_name']
        self._create_drop_db(cfg, sql)
        subprocess.run(self.db_schema_cmd(cfg))


class EndTestDatabase(AbstractTestDatabase):
    def db_schema_cmd(self, cfg):
        out = super().db_schema_cmd(cfg)
        out.extend(['rollback', '0.0.0'])
        return out

    def setup_test_db(self, cfg):
        subprocess.run(self.db_schema_cmd(cfg))
        sql = 'DROP DATABASE `%s`;' % cfg['db_name']
        self._create_drop_db(cfg, sql)


if __name__ == '__main__':
    assert len(sys.argv) >= 3, "arguments must include (1)dotted path to renewal handler class (2) corresponding configuration file "
    class_path   = sys.argv[-3]
    cfg_filepath = sys.argv[-2]
    liquibase_path = sys.argv[-1]
    cls = import_module_string(dotted_path=class_path)
    cls().start(cfg_filepath, liquibase_path)