import copy
import json
import random
from functools import partial

from django.db.utils import IntegrityError, DataError
from django.contrib.contenttypes.models  import ContentType
from django.core.exceptions    import ValidationError as DjangoValidationError
from rest_framework.exceptions import ValidationError as DRFValidationError

from common.util.python import flatten_nested_iterable, sort_nested_object
from product.models.base import _ProductAttrValueDataType, ProductSaleableItem
from product.models.development import ProductDevIngredientType

_fixtures = {
    'AuthUser': [
        {'id':14, 'is_staff':True,  'is_active':True,  'username': 'yusir001','password': '93rutGrPt'} ,
        {'id':19, 'is_staff':False, 'is_active':True,  'username': 'yusir002','password': '39rjfrret'} ,
        {'id':10, 'is_staff':True,  'is_active':False, 'username': 'yusir003','password': 'if74w#gfy'} ,
    ],
    'ProductTag': [
        {'id':30 , 'usrprof': 56,'name':'Food & Beverage'}               ,
        {'id':31 , 'usrprof': 56,'name':'DIY hardware'}                  ,
        {'id':32 , 'usrprof': 56,'name':'Dairy'}                         ,
        {'id':33 , 'usrprof': 56,'name':'Farm Produce'}                  ,
        {'id':34 , 'usrprof': 56,'name':'Embedded system device'}        ,
        {'id':35 , 'usrprof': 56,'name':'semi-prepared food ingredient'} ,
        {'id':36 , 'usrprof': 56,'name':'Veggie'}                        ,
        {'id':37 , 'usrprof': 56,'name':'Fruit'}                         ,
        {'id':38 , 'usrprof': 56,'name':'Debugging device'}              ,
    ],
    'ProductTagClosure': [
        {'id':1, 'ancestor':30, 'descendant':30, 'depth':0},
        {'id':2, 'ancestor':31, 'descendant':31, 'depth':0},
        {'id':3, 'ancestor':32, 'descendant':32, 'depth':0},
        {'id':4, 'ancestor':33, 'descendant':33, 'depth':0},
        {'id':5, 'ancestor':34, 'descendant':34, 'depth':0},
        {'id':6, 'ancestor':35, 'descendant':35, 'depth':0},
        {'id':7, 'ancestor':36, 'descendant':36, 'depth':0},
        {'id':8, 'ancestor':37, 'descendant':37, 'depth':0},
        {'id':9, 'ancestor':30, 'descendant':32, 'depth':1},
        {'id':10, 'ancestor':30, 'descendant':33, 'depth':1},
        {'id':11, 'ancestor':30, 'descendant':35, 'depth':1},
        {'id':12, 'ancestor':33, 'descendant':36, 'depth':1},
        {'id':13, 'ancestor':33, 'descendant':37, 'depth':1},
        {'id':14, 'ancestor':30, 'descendant':36, 'depth':2},
        {'id':15, 'ancestor':30, 'descendant':37, 'depth':2},
        {'id':16, 'ancestor':31, 'descendant':34, 'depth':1},
        {'id':17, 'ancestor':38, 'descendant':38, 'depth':0},
        {'id':18, 'ancestor':31, 'descendant':38, 'depth':1},
    ],
    'ProductAttributeType': [
        {'id':20, 'name': 'toppings category', 'dtype': _ProductAttrValueDataType.STRING.value[0][0]},
        {'id':21, 'name': 'color', 'dtype': _ProductAttrValueDataType.STRING.value[0][0]},
        {'id':22, 'name': 'bread crust level', 'dtype': _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]},
        {'id':23, 'name': 'cache size (KBytes)', 'dtype': _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]},
        {'id':24, 'name': 'min. working temperature (celsius)', 'dtype': _ProductAttrValueDataType.INTEGER.value[0][0]},
        {'id':25, 'name': 'min. dormant temperature (celsius)', 'dtype': _ProductAttrValueDataType.INTEGER.value[0][0]},
        {'id':26, 'name': 'Length of square (Ft.)', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
        {'id':27, 'name': 'Diameter (In.)', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
        {'id':28, 'name': 'max resistence voltage', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
        {'id':29, 'name': 'min distance between 2 metal wires (um)', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
    ],
    'ProductAttributeValueStr': ['sticky', 'crunchy', 'chubby', 'chewy', 'crispy', 'meaty', 'creepy'],
    'ProductAttributeValuePosInt': [random.randrange(1,10000) for _ in range(25)],
    'ProductAttributeValueInt': [random.randrange(-10000,10000) for _ in range(25)],
    'ProductAttributeValueFloat': [random.randrange(-100,100) * 0.31415926 for _ in range(25)],
    'ProductAppliedAttributePrice': [10.4 , 59, 80.3, 19.4, 94.2, 13.4, 5.67, 88.9],
    'ProductDevIngredient': [
        {'id':2, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'tomato'},
        {'id':3, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'all-purpose flour'},
        {'id':4, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'bread flour'},
        {'id':5, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'quail egg'},
        {'id':6, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'dry yeast powder'},
        {'id':7, 'category':ProductDevIngredientType.WORK_IN_PROGRESS, 'name':'poolish'},
        {'id':8, 'category':ProductDevIngredientType.WORK_IN_PROGRESS, 'name':'tomato puree'},
        {'id':9, 'category':ProductDevIngredientType.WORK_IN_PROGRESS, 'name':'beef bone broth'},
        {'id':10, 'category':ProductDevIngredientType.WORK_IN_PROGRESS, 'name':'LiPo Battery'},
        {'id':11, 'category':ProductDevIngredientType.WORK_IN_PROGRESS  , 'name':'RISC-V SoC'},
        {'id':12, 'category':ProductDevIngredientType.WORK_IN_PROGRESS  , 'name':'ARM Cortex-A72 SoC'},
        {'id':13, 'category':ProductDevIngredientType.WORK_IN_PROGRESS  , 'name':'Pixhawk flight controller'},
        {'id':14, 'category':ProductDevIngredientType.WORK_IN_PROGRESS  , 'name':'GPS sensor'},
        {'id':15, 'category':ProductDevIngredientType.WORK_IN_PROGRESS  , 'name':'USB chip driver'},
        {'id':16, 'category':ProductDevIngredientType.CONSUMABLES     , 'name':'bio gas'},
        {'id':17, 'category':ProductDevIngredientType.EQUIPMENTS      , 'name':'oven'},
        {'id':18, 'category':ProductDevIngredientType.EQUIPMENTS  , 'name':'Soldering kit'},
        {'id':19, 'category':ProductDevIngredientType.EQUIPMENTS  , 'name':'Portable Oscilloscope'},
        {'id':20, 'category':ProductDevIngredientType.EQUIPMENTS  , 'name':'Logic Analyzer'},
    ],
    'ProductSaleableItemMedia': [
        {'media':'384gaeirj4jg393P'},
        {'media':'92u4t09u4tijq3oti'},
        {'media':'2903tijtg3h4teg'},
        {'media':'09fawgsdkmbiehob'},
        {'media':'2093jti4jt0394ut'},
        {'media':'0fwkbb0erwrwXrqrt'},
        {'media':'309ur204t42jWh1'},
        {'media':'eOy1r0j4SKuAYEre'},
    ],
    'ProductSaleableItem': [
        {'visible':  True, 'name':'Raspberry PI 4 Dev board', 'price':3.88,  'usrprof':19},
        {'visible': False, 'name':'SiFive HiFive Unmatched' , 'price':11.30, 'usrprof':212},
        {'visible':  True, 'name':'rough rice noOdle',   'price':0.18,  'usrprof':212},
        {'visible': False, 'name':'Mozzarella pizza', 'price':13.93, 'usrprof':79},
        {'visible':  True, 'name':'Pita dough', 'price':3.08, 'usrprof':79},
        {'visible': False, 'name':'quad drone', 'price':17.02, 'usrprof':212},
        {'visible':  True, 'name':'Industrial Fan', 'price':29.10, 'usrprof':53},
        {'visible': False, 'name':'Trail runner shoes', 'price': 69.9, 'usrprof':79},
        {'visible':  True, 'name':'concrete soil', 'price': 5.6, 'usrprof':28},
        {'visible': False, 'name':'Banana PI dev board', 'price':1.04,  'usrprof':53},
        {'visible':  True, 'name':'Semi-prepared Beef Noodle Soup',  'price':6.49,  'usrprof':53},
    ],
} # end of _fixtures


http_request_body_template = {
    'ProductDevIngredient': {
        'name': None,  'id': None, 'category': None,
        'attributes':[
            #{'id':None, 'type':None, 'value': None},
        ],
    },
    'ProductSaleableItem': {
        'name': None,  'id': None, 'visible': None, 'price': None,
        'tags':[] ,
        'media_set':[],
        'attributes':[
            #{'id':None, 'type':None, 'value': None, 'extra_amount':None},
        ],
        'ingredients_applied': [
            #{'ingredient': None, 'unit': None, 'quantity': None},
        ]
    } # end of ProductSaleableItem
} # end of http_request_body_template


_load_init_params = lambda init_params, model_cls: model_cls(**init_params)

_modelobj_list_to_map = lambda list_: {item.pk: item for item in list_}

_dict_key_replace = lambda obj, from_, to_: {to_ if k == from_ else k: v for k,v in obj.items()}

_dict_kv_pair_evict = lambda obj, cond_fn: dict(filter(cond_fn, obj.items()))


def listitem_rand_assigner(list_, min_num_chosen:int=2, max_num_chosen:int=-1, distinct:bool=True):
    # utility for testing
    assert any(list_), 'input list should not be empty'
    assert min_num_chosen >= 0, 'min_num_chosen = %s' % min_num_chosen
    num_avail = len(list_)
    if max_num_chosen > 0:
        err_msg = 'max_num_chosen = %s, min_num_chosen = %s' % (max_num_chosen, min_num_chosen)
        assert max_num_chosen > min_num_chosen, err_msg
        if max_num_chosen > (num_avail + 1) and distinct is True:
            err_msg = 'num_avail = %s, max_num_chosen = %s, distinct = %s' \
                    % (num_avail, max_num_chosen, distinct)
            raise ValueError(err_msg)
    else:
        err_msg =  'num_avail = %s , min_num_chosen = %s' % (num_avail, min_num_chosen)
        assert num_avail >= min_num_chosen, err_msg
        max_num_chosen = num_avail + 1
    if distinct:
        list_ = list(list_)
    num_assigned = random.randrange(min_num_chosen, max_num_chosen)
    for _ in range(num_assigned):
        idx = random.randrange(num_avail)
        yield list_[idx]
        if distinct:
            num_avail -= 1
            del list_[idx]
## end of listitem_rand_assigner


def _common_instances_setup(out:dict, models_info):
    """ create instances of given model classes in Django ORM """
    for model_cls, num_instance_required in models_info:
        bound_fn = partial(_load_init_params, model_cls=model_cls)
        model_name = model_cls.__name__
        ##params = _fixtures[model_name][:num_instance_required]
        params_gen = listitem_rand_assigner(list_=_fixtures[model_name],
                min_num_chosen=num_instance_required,
                max_num_chosen=(num_instance_required + 1))
        objs = list(map(bound_fn, params_gen))
        model_cls.objects.bulk_create(objs)
        out[model_name] = list(model_cls.objects.all())


def rand_gen_request_body(template, customize_item_fn, data_gen):
    def rand_gen_single_req(data):
        single_req_item = copy.deepcopy(template)
        single_req_item.update(data)
        customize_item_fn(single_req_item)
        return single_req_item
    return map(rand_gen_single_req, data_gen)


def _get_inst_attr(obj, attname, default_value=None):
    if isinstance(obj, dict):
        out = obj.get(attname, default_value)
    else:
        out = getattr(obj, attname, default_value)
    return out


def assert_field_equal(fname, testcase, expect_obj, actual_obj):
    expect_val = _get_inst_attr(expect_obj,fname)
    actual_val = _get_inst_attr(actual_obj,fname)
    testcase.assertEqual(expect_val, actual_val)


def _null_test_obj_attrs(testcase, instance, field_names):
    for fname in field_names:
        old_value = getattr(instance, fname)
        setattr(instance, fname, None)
        with testcase.assertRaises(IntegrityError) as e:
            instance.save(force_insert=True)
        setattr(instance, fname, old_value)


def _gen_ingredient_attrvals(attrtype_ref, ingredient, idx, extra_charge=None):
    ingredient_ct = ContentType.objects.get_for_model(ingredient)
    model_cls = attrtype_ref.attr_val_set.model
    num_limit = len(_fixtures[model_cls.__name__])
    new_value = _fixtures[model_cls.__name__][idx % num_limit]
    model_init_kwargs = {
        'ingredient_type': ingredient_ct,  'ingredient_id':ingredient.pk,
        'attr_type':attrtype_ref, 'value':new_value
    }
    if extra_charge and extra_charge > 0.0:
        model_init_kwargs['extra_amount'] = extra_charge
    return model_cls(**model_init_kwargs)


def _ingredient_attrvals_common_setup(attrtypes_gen_fn, ingredients):
    _attrval_objs = {item[0][0]:{} for item in _ProductAttrValueDataType}
    idx = 0
    for ingredient in ingredients:
        attrtypes_gen = attrtypes_gen_fn()
        for attrtype_ref in attrtypes_gen:
            if _attrval_objs[attrtype_ref.dtype].get(ingredient.pk) is None:
                _attrval_objs[attrtype_ref.dtype][ingredient.pk] = []
            attrval = _gen_ingredient_attrvals(attrtype_ref, ingredient, idx)
            _attrval_objs[attrtype_ref.dtype][ingredient.pk].append(attrval)
            idx += 1
    for dtype, objmap in _attrval_objs.items():
        related_field_name = _ProductAttrValueDataType.related_field_map(dtype_code=dtype)
        related_field_mgr = getattr(ingredients[0], related_field_name)
        objs = tuple(flatten_nested_iterable(list_=[x for x in objmap.values()]))
        related_field_mgr.bulk_create(objs)
    return _attrval_objs


def reset_serializer_validation_result(serializer):
    serializer._errors.clear()
    delattr(serializer, '_validated_data')


class HttpRequestDataGen:
    def customize_req_data_item(self, item, **kwargs):
        raise NotImplementedError()


class AttributeDataGenMixin:
    min_num_applied_attrs = 0
    max_num_applied_attrs = len(_fixtures['ProductAttributeType'])

    def _gen_attr_val(self, attrtype, extra_amount_enabled):
        model_fixtures = _fixtures
        nested_item = {'id':None, 'type':_get_inst_attr(attrtype,'id'), 'value': None,}
        _fn = lambda option: option.value[0][0] == _get_inst_attr(attrtype,'dtype')
        dtype_option = filter(_fn, _ProductAttrValueDataType)
        dtype_option = tuple(dtype_option)[0]
        field_name = dtype_option.value[0][1]
        field_descriptor = getattr(ProductSaleableItem, field_name)
        attrval_cls_name = field_descriptor.field.related_model.__name__
        value_list = model_fixtures[attrval_cls_name]
        chosen_idx = random.randrange(0, len(value_list))
        nested_item['value'] = value_list[chosen_idx]
        rand_enable_extra_amount = random.randrange(0, 2)
        if extra_amount_enabled and rand_enable_extra_amount > 0:
            extra_amount_list = model_fixtures['ProductAppliedAttributePrice']
            chosen_idx = random.randrange(0, len(extra_amount_list))
            nested_item['extra_amount'] = float(extra_amount_list[chosen_idx])
        return nested_item

    def gen_attr_vals(self, extra_amount_enabled):
        num_attrvals    = random.randrange(self.min_num_applied_attrs, self.max_num_applied_attrs)
        attr_dtypes_gen = listitem_rand_assigner(list_=_fixtures['ProductAttributeType'],
                min_num_chosen=num_attrvals, max_num_chosen=(num_attrvals + 1))
        bound_gen_attr_val = partial(self._gen_attr_val, extra_amount_enabled=extra_amount_enabled)
        return list(map(bound_gen_attr_val, attr_dtypes_gen))
## end of class AttributeDataGenMixin


class BaseVerificationMixin:
    serializer_class = None

    def _get_non_nested_fields(self, skip_id=True):
        check_fields = copy.copy(self.serializer_class.Meta.fields)
        if skip_id:
            check_fields.remove('id')
        return check_fields

    def _assert_simple_fields(self, check_fields,  exp_sale_item, ac_sale_item):
        self.assertNotEqual(_get_inst_attr(ac_sale_item,'id'), None)
        self.assertGreater(_get_inst_attr(ac_sale_item,'id'), 0)
        bound_assert_fn = partial(assert_field_equal, testcase=self,  expect_obj=exp_sale_item, actual_obj=ac_sale_item)
        tuple(map(bound_assert_fn, check_fields))

    def _assert_serializer_validation_error(self, testcase, serializer, reset_validation_result=True):
        error_details = None
        possible_exception_classes = (DjangoValidationError, DRFValidationError, AssertionError)
        with testcase.assertRaises(possible_exception_classes):
            try:
                serializer.is_valid(raise_exception=True)
            except possible_exception_classes as e:
                error_details = e.detail
                raise
            finally:
                if reset_validation_result:
                    reset_serializer_validation_result(serializer=serializer)
        testcase.assertNotEqual(error_details, None)
        return error_details

    def _assert_single_invalid_case(self, testcase, field_name, invalid_value, expect_err_msg,
            req_data, serializer, fn_choose_edit_item):
        origin_value = req_data[field_name]
        req_data[field_name] = invalid_value
        origin_error_details = self._assert_serializer_validation_error(testcase=testcase, serializer=serializer)
        req_data[field_name] = origin_value
        error_details = fn_choose_edit_item(origin_error_details)
        error_details = error_details[field_name]
        testcase.assertGreaterEqual(len(error_details), 1)
        actual_err_msg = str(error_details[0])
        testcase.assertEqual(expect_err_msg, actual_err_msg)
        return origin_error_details

    def verify_objects(self, actual_instances, expect_data,  **kwargs):
        raise NotImplementedError()

    def verify_data(self, actual_data, expect_data, **kwargs):
        raise NotImplementedError()
## end of class BaseVerificationMixin


class AttributeAssertionMixin:

    def _assert_attributes_data_change(self, data_before_validate, data_after_validate, skip_attr_val_id=False):
        # check data conversion between `attributes` field and internal attribute fields for specific data type
        def _flatten_attr_vals(item):
            evict_condition = lambda kv: kv[0] not in ('ingredient_type', 'ingredient_id')
            attrtype_key_replace_fn = partial(_dict_key_replace, to_='type', from_='attr_type')
            ingredient_ctype_kv_evict_fn = partial(_dict_kv_pair_evict, cond_fn=evict_condition)
            flattened_attrvals = [attr_val for dtype_opt in _ProductAttrValueDataType \
                    for attr_val in item.get(dtype_opt.value[0][1], [])]
            flattened_attrvals = map(attrtype_key_replace_fn, flattened_attrvals)
            flattened_attrvals = map(ingredient_ctype_kv_evict_fn, flattened_attrvals)
            return list(flattened_attrvals)

        def _evict_attr_val_id_fn(item):
            evict_condition = lambda kv: kv[0] != 'id'
            id_evict_fn = partial(_dict_kv_pair_evict, cond_fn=evict_condition)
            item = map(id_evict_fn, item)
            return list(item)

        attrs_before_validate = list(map(lambda x:x['attributes'], data_before_validate))
        attrs_after_validate  = list(map(_flatten_attr_vals, data_after_validate))
        if skip_attr_val_id:
            attrs_before_validate = list(map(_evict_attr_val_id_fn, attrs_before_validate))
            attrs_after_validate  = list(map(_evict_attr_val_id_fn, attrs_after_validate ))
        attrs_before_validate = sort_nested_object(obj=attrs_before_validate)
        attrs_after_validate  = sort_nested_object(obj=attrs_after_validate)
        expect_vals = json.dumps(attrs_before_validate, sort_keys=True)
        actual_vals = json.dumps(attrs_after_validate , sort_keys=True)
        self.assertEqual(expect_vals, actual_vals)


    def _assert_product_attribute_fields(self, exp_sale_item, ac_sale_item):
        # TODO, rename to `cpompare_attribute_fields`
        key_evict_condition = lambda kv: (kv[0] not in ('id', 'ingredient_type', 'ingredient_id')) \
                and not (kv[0] == 'extra_amount' and kv[1] is None)
        bound_dict_kv_pair_evict = partial(_dict_kv_pair_evict,  cond_fn=key_evict_condition)
        bound_dict_key_replace = partial(_dict_key_replace, to_='extra_amount', from_='_extra_charge__amount')
        # compare attribute values based on its data type
        for dtype_option in _ProductAttrValueDataType:
            field_name = dtype_option.value[0][1]
            expect_vals = exp_sale_item.get(field_name, None)
            if not expect_vals:
                continue
            expect_vals = list(map(bound_dict_kv_pair_evict, expect_vals))
            manager = _get_inst_attr(ac_sale_item, field_name)
            actual_vals = manager.values('attr_type', 'value', '_extra_charge__amount')
            actual_vals = map(bound_dict_key_replace, actual_vals)
            actual_vals = list(map(bound_dict_kv_pair_evict, actual_vals))
            expect_vals = sorted(expect_vals, key=lambda x:x['attr_type'])
            actual_vals = sorted(actual_vals, key=lambda x:x['attr_type'])
            expect_vals = json.dumps(expect_vals, sort_keys=True)
            actual_vals = json.dumps(actual_vals, sort_keys=True)
            self.assertEqual(expect_vals, actual_vals)
        # compare attribute values in `attributes` field
        ## exp_attrs = _get_inst_attr(exp_sale_item, 'attributes', [])
        ## ac_attrs  = _get_inst_attr(ac_sale_item,  'attributes', [])
        ## exp_attrs = sort_nested_object(obj=exp_attrs)
        ## ac_attrs  = sort_nested_object(obj=ac_attrs )
        ## exp_attrs = json.dumps(exp_attrs, sort_keys=True)
        ## ac_attrs  = json.dumps(ac_attrs , sort_keys=True)
        ## if exp_attrs != ac_attrs:
        ##     import pdb
        ##     pdb.set_trace()
        ## self.assertEqual(exp_attrs, ac_attrs)


    def _test_unclassified_attribute_error(self, testcase, serializer, request_data,
            assert_single_invalid_case_fn):
        invalid_cases = [
            ('type',  None, 'unclassified attribute type `None`'),
            ('type', 'Lo0p','unclassified attribute type `Lo0p`'),
            ('type',  9999, 'unclassified attribute type `9999`'),
            ('type',  99.9, 'unclassified attribute type `99.9`'),
        ]
        request_data = list(filter(lambda d: any(d['attributes']), request_data))
        for field_name, invalid_value, expect_err_msg in invalid_cases:
            for idx in range(len(request_data)):
                # serializer data has to be entirely reset for next iteration because it
                # reports the validation error for all list items in one go
                serializer.initial_data = copy.deepcopy(request_data)
                jdx = random.randrange(0, len(serializer.initial_data[idx]['attributes']))
                fn_choose_edit_item = lambda x : x[idx]['attributes'][jdx]
                req_data = fn_choose_edit_item(serializer.initial_data)
                assert_single_invalid_case_fn(testcase=testcase, field_name=field_name, req_data=req_data,
                        invalid_value=invalid_value, expect_err_msg=expect_err_msg, serializer=serializer,
                        fn_choose_edit_item=fn_choose_edit_item)


    def _test_unclassified_attributes_error(self, serializer, request_data, testcase,
            assert_serializer_validation_error_fn, num_rounds=10, extra_invalid_cases=None,
            field_name='type', expect_err_msg_pattern='unclassified attribute type `%s`'):
        request_data = list(filter(lambda d: any(d['attributes']), request_data))
        extra_invalid_cases = extra_invalid_cases or ()
        invalid_cases = ( 9999, '9q98', 9997,) + extra_invalid_cases
        num_invalid_cases = len(invalid_cases)
        for _ in range(num_rounds):
            serializer.initial_data = copy.deepcopy(request_data)
            invalid_cases_iter = iter(invalid_cases)
            idx_to_attrs = {}
            while len(idx_to_attrs.keys()) < num_invalid_cases:
                idx = random.randrange(0, len(request_data))
                jdx = random.randrange(0, len(request_data[idx]['attributes']))
                if idx_to_attrs.get((idx,jdx)) is None:
                    idx_to_attrs[(idx,jdx)] = next(invalid_cases_iter)
            fn_choose_edit_item = lambda x, idx, jdx : x[idx]['attributes'][jdx]
            for key, invalid_value in idx_to_attrs.items():
                req_data = fn_choose_edit_item(serializer.initial_data, key[0], key[1])
                req_data[field_name] = invalid_value
            error_details = assert_serializer_validation_error_fn(testcase=testcase, serializer=serializer)
            # the number of error details varies because django reports only one error
            # at a time even there are multiple errors in the serialized data , this test
            # only ensures at least one error(s) can be reported by Django.
            num_errors_catched = 0
            for key, invalid_value in idx_to_attrs.items():
                error_detail = fn_choose_edit_item(error_details, key[0], key[1])
                if not error_detail:
                    continue
                error_detail = error_detail[field_name]
                testcase.assertGreaterEqual(len(error_detail), 1)
                actual_err_msg = str(error_detail[0])
                expect_err_msg = expect_err_msg_pattern % invalid_value
                testcase.assertEqual(expect_err_msg, actual_err_msg)
                num_errors_catched += 1
            testcase.assertGreaterEqual(num_errors_catched, 1)
            testcase.assertLessEqual(num_errors_catched, num_invalid_cases)
    ## end of _test_unclassified_attributes_error()


    def _test_incorrect_attribute_value(self, serializer, request_data, testcase, assert_serializer_validation_error_fn,
            num_rounds=10, field_name='value'):
        _attr_fixture = {
             'null' :( None, 'unclassified attribute type `None`'),
             _ProductAttrValueDataType.STRING.value[0][0]           :'Lo0p',
             _ProductAttrValueDataType.INTEGER.value[0][0]          : -999 ,
             _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0] : 9999 ,
             _ProductAttrValueDataType.FLOAT.value[0][0]            : 99.9 ,
        }
        _allowed_type_transitions = [
            (_ProductAttrValueDataType.FLOAT.value[0][0], _ProductAttrValueDataType.INTEGER.value[0][0]),
            (_ProductAttrValueDataType.FLOAT.value[0][0], _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]),
            (_ProductAttrValueDataType.INTEGER[0][0], _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]),
            (_ProductAttrValueDataType.STRING.value[0][0], _ProductAttrValueDataType.INTEGER.value[0][0]         ),
            (_ProductAttrValueDataType.STRING.value[0][0], _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]),
            (_ProductAttrValueDataType.STRING.value[0][0], _ProductAttrValueDataType.FLOAT.value[0][0]           ),
        ]
        expect_err_code = ('null', 'invalid', 'min_value')
        num_attr_fixture = len(_attr_fixture)
        request_data = list(filter(lambda d: any(d['attributes']), request_data))
        for _ in range(num_rounds):
            serializer.initial_data = copy.deepcopy(request_data)
            idx_to_attrs = {}
            while len(idx_to_attrs.keys()) < num_attr_fixture:
                idx = random.randrange(0, len(serializer.initial_data))
                jdx = random.randrange(0, len(serializer.initial_data[idx]['attributes']))
                if idx_to_attrs.get((idx, jdx)) is None:
                    attrtype_id = serializer.initial_data[idx]['attributes'][jdx]['type']
                    attrtype = filter(lambda obj: obj.id == attrtype_id, testcase.stored_models['ProductAttributeType'])
                    attrtype = tuple(attrtype)[0]
                    dtype_keys = list(_attr_fixture.keys())
                    dtype_keys.remove(attrtype.dtype) # create invalid case by giving different data type of value
                    chosen_key = random.choice(dtype_keys)
                    if (attrtype.dtype, chosen_key) not in _allowed_type_transitions:
                        idx_to_attrs[(idx, jdx)] = (attrtype.dtype, chosen_key)
            fn_choose_edit_item = lambda x, idx, jdx : x[idx]['attributes'][jdx]
            for key, invalid_value in idx_to_attrs.items():
                req_data = fn_choose_edit_item(serializer.initial_data, key[0], key[1])
                req_data[field_name] = _attr_fixture[invalid_value[1]]
            error_details =  assert_serializer_validation_error_fn(testcase=testcase, serializer=serializer)
            for key, transition in idx_to_attrs.items():
                error_detail = fn_choose_edit_item(error_details, key[0], key[1])
                #if not error_detail:
                #    import pdb
                #    pdb.set_trace()
                testcase.assertTrue(any(error_detail))
                error_detail = error_detail[field_name]
                testcase.assertGreaterEqual(len(error_detail), 1)
                actual_err_code = error_detail[0].code
                testcase.assertIn(actual_err_code, expect_err_code)
## end of class AttributeAssertionMixin


