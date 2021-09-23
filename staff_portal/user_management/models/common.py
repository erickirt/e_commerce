from functools import partial
from django.db  import  transaction
from common.util.python.django.setup import test_enable as django_test_enable

DB_ALIAS_APPLIED = 'default' if django_test_enable else 'usermgt_service'
# note that atomicity fails siliently with incorrect database credential
# that is why I use partial() to tie `using` argument with transaction.atomic(**kwargs)
_atomicity_fn = partial(transaction.atomic, using=DB_ALIAS_APPLIED)

