import copy

from django.db.models.constants import LOOKUP_SEP
from rest_framework.permissions import BasePermission as DRFBasePermission
from rest_framework.filters import BaseFilterBackend as DRFBaseFilterBackend

from common.auth.jwt import JWTclaimPermissionMixin

class AppBasePermission(DRFBasePermission, DRFBaseFilterBackend, JWTclaimPermissionMixin):
    def has_permission(self, request, view):
        return self._has_permission(tok_payld=request.auth, method=request.method)

    def filter_queryset(self, request, queryset, view):
        return queryset
## end of class AppBasePermission


class InputDataOwnerPermissionMixin:
    # currently the function only supports single-column key ID
    def _http_put_delete_permission(self, request, view, id_label='id'):
        result = False
        fn = lambda x: x.get(id_label, None)
        edit_ids = filter(fn, request.data)
        edit_ids = set(map(fn, edit_ids))
        if any(edit_ids):
            profile_id = request.user.profile
            try:
                lookup_kwargs = {LOOKUP_SEP.join([id_label,'in']):edit_ids, 'usrprof':profile_id}
                qset = view.queryset.filter(**lookup_kwargs)
                if qset.count() == len(edit_ids):
                    result = True
            except ValueError as e: # TODO, log the error 
                # comes from invalid data type of ID, skip it for now,
                # later the serializer will check the ID again then
                # raise validation error and respond with 400 bad request
                pass
        return result

    def _input_data_owner_check(self, request, view):
        # TODO, consider the data which can be read only by small portion of clients
        if request.method.lower() in ('get', 'post', 'patch',):
            result = True
        elif request.method.lower() in ('put', 'delete',):
            result = self._http_put_delete_permission(request, view)
        else:
            result = False
        return result


class TagsPermissions(AppBasePermission):
    perms_map = {
        'GET':    [],
        'OPTIONS': [],
        'HEAD':   [],
        'POST':   ['view_producttag', 'add_producttag'],
        'PUT':    ['view_producttag', 'change_producttag'],
        'PATCH':  ['view_producttag', 'change_producttag'],
        'DELETE': ['view_producttag', 'delete_producttag'],
    }

class AttributeTypePermissions(AppBasePermission):
    perms_map = {
        'GET': [],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   ['view_productattributetype', 'add_productattributetype'],
        'PUT':    ['view_productattributetype', 'change_productattributetype'],
        'PATCH':  ['view_productattributetype', 'change_productattributetype'],
        'DELETE': ['view_productattributetype', 'delete_productattributetype'],
    }


class FabricationIngredientPermissions(AppBasePermission):
    perms_map = {
        'GET':  ['view_productdevingredient'],
        'OPTIONS': [],
        'HEAD':  [],
        'POST':   ['view_productdevingredient', 'add_productdevingredient',   ],
        'PUT':    ['view_productdevingredient', 'change_productdevingredient',],
        'PATCH':  ['view_productdevingredient', 'change_productdevingredient',],
        'DELETE': ['view_productdevingredient', 'delete_productdevingredient',],
    }

class SaleableItemPermissions(AppBasePermission, InputDataOwnerPermissionMixin):
    perms_map = {
        'GET':  [], # TODO, skip applied ingredients if the user doesn't have the view permission on that
        'OPTIONS': [],
        'HEAD': [],
        'POST':   ['view_productsaleableitem', 'add_productsaleableitem',   ],
        'PUT':    ['view_productsaleableitem', 'change_productsaleableitem',],
        'PATCH':  ['view_productsaleableitem', 'change_productsaleableitem',],
        'DELETE': ['view_productsaleableitem', 'delete_productsaleableitem',],
        # users should have access to change delete status, since its soft-delete
    }
    def has_permission(self, request, view):
        result = super().has_permission(request=request, view=view)
        if result is True:
            result = self._input_data_owner_check(request, view)
        return result
## end of class SaleableItemPermissions


class SaleablePackagePermissions(AppBasePermission, InputDataOwnerPermissionMixin):
    perms_map = {
        'GET':  [],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   ['view_productsaleablepackage', 'add_productsaleablepackage',  ],
        'PUT':    ['view_productsaleablepackage', 'change_productsaleablepackage'],
        'PATCH':  ['view_productsaleablepackage', 'change_productsaleablepackage'],
        'DELETE': ['view_productsaleablepackage', 'delete_productsaleablepackage'],
    }

    def has_permission(self, request, view):
        result = super().has_permission(request=request, view=view)
        if result is True:
            result = self._input_data_owner_check(request, view)
        return result
## end of class SaleablePackagePermissions

