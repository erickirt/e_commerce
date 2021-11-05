# Generated by Django 3.1 on 2021-10-07 07:43
from django.db import migrations, connections
from django.contrib.contenttypes.models import ContentType

from common.util.python import flatten_nested_iterable
from common.util.python.django.setup import test_enable as django_test_enable

from product.models.base import ProductTag, ProductSaleableItem, ProductSaleablePackage, ProductAttributeType
from product.models.development import ProductDevIngredient


def _render_perms_sql(model_cls):
    actions = ('add', 'change', 'delete', 'view')
    insert_pattern = 'INSERT INTO `auth_permission` (`name`, `codename`, `content_type_id`) VALUES (\'%s\', \'%s\', (SELECT `id` FROM `{db_table}` WHERE `app_label` = \'{app_label}\' AND `model` = \'%s\'))'
    delete_pattern = 'DELETE FROM `auth_permission` WHERE `content_type_id` IN (SELECT `id` FROM `{db_table}` WHERE `app_label` = \'{app_label}\' AND `model` = \'%s\')'
    insert_pattern = insert_pattern.format( db_table=ContentType._meta.db_table,  app_label=model_cls._meta.app_label )
    delete_pattern = delete_pattern.format( db_table=ContentType._meta.db_table,  app_label=model_cls._meta.app_label )
    ops = []
    model_name = model_cls._meta.model_name
    for action in actions:
        description = 'Can %s %s' % (action, model_cls._meta.verbose_name)
        codename = '%s_%s' % (action, model_name)
        op = migrations.RunSQL(
               sql=insert_pattern % (description, codename, model_name),
               # the smae delete SQL can be performed multiple times without error
               reverse_sql=delete_pattern % (model_name)
           )
        ops.append(op)
    return ops

def _render_quota_material_sql(model_cls, app_code=2):
    insert_pattern = 'INSERT INTO `quota_material` (`app_code`, `mat_code`) VALUES (%s, %s)'
    delete_pattern = 'DELETE FROM `quota_material` WHERE `app_code` = %s AND `mat_code` = %s'
    mat_code = model_cls.quota_material.value
    op = migrations.RunSQL(
           sql=insert_pattern % (app_code, mat_code),
           reverse_sql=delete_pattern % (app_code, mat_code)
       )
    return op


class Migration(migrations.Migration):
    dependencies = [
        ('product', '0002_rawsqls'),
    ]
    operations = []

    def __new__(cls, name, app_label, *args, **kwargs):
        if not django_test_enable and not any(cls.operations):
            cls._load_ops() # all operations in this file must NOT be applied to test database
        return super().__new__(cls, *args, **kwargs)

    @classmethod
    def _load_ops(cls):
        auth_classes = (ProductTag, ProductSaleableItem, ProductSaleablePackage, ProductAttributeType, ProductDevIngredient)
        quota_mat_classes = (ProductSaleableItem, ProductSaleablePackage,)
        insert_contenttype_pattern = 'INSERT INTO `{db_table}` (`app_label`, `model`) VALUES (\'{app_label}\', \'%s\')'
        insert_contenttype_pattern = insert_contenttype_pattern.format(db_table=ContentType._meta.db_table, app_label=ProductSaleableItem._meta.app_label)
        delete_contenttype_pattern = 'DELETE FROM `{db_table}` WHERE `app_label` = \'{app_label}\' AND `model` = \'%s\''
        delete_contenttype_pattern = delete_contenttype_pattern.format(db_table=ContentType._meta.db_table, app_label=ProductSaleableItem._meta.app_label)
        extra_ops = [
            migrations.RunSQL(
                sql=insert_contenttype_pattern % model_cls._meta.model_name,
                reverse_sql=delete_contenttype_pattern % model_cls._meta.model_name
            ) for model_cls in auth_classes
        ]
        cls.operations.extend(extra_ops)

        extra_ops = map(_render_perms_sql, auth_classes)
        extra_ops = flatten_nested_iterable(list_=extra_ops)
        extra_ops = list(extra_ops)
        cls.operations.extend(extra_ops)

        extra_ops = list(map(_render_quota_material_sql, quota_mat_classes))
        cls.operations.extend(extra_ops)
    ## end of _load_ops
## end of class Migration

