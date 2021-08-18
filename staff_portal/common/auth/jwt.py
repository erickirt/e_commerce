from datetime import datetime, timedelta, timezone
from functools import partial
import math
import logging
import jwt
from jwt import PyJWKClient
from jwt.api_jwk import PyJWK
from jwt.utils import to_base64url_uint
from jwt.exceptions import PyJWKClientError

from common.util.python  import ExtendedDict
from common.auth.keystore import AbstractKeystorePersistReadMixin, RSAKeygenHandler
_logger = logging.getLogger(__name__)


class JWT:
    """
    internal wrapper class for detecting JWT write, verify, and generate encoded token,
    in this wrapper, `acc_id` claim is required in payload
    """

    def __init__(self, encoded=None):
        self.encoded = encoded
        self._destroy = False
        self._valid = None

    @property
    def encoded(self):
        return self._encoded

    @encoded.setter
    def encoded(self, value):
        self._encoded = value
        if value:
            header = jwt.get_unverified_header(value)
            payld  = jwt.decode(value, options={'verify_signature':False})
        else:
            header = {}
            payld  = {}
        self._payld  = ExtendedDict(payld)
        self._header = ExtendedDict(header)

    @property
    def payload(self):
        return self._payld

    @property
    def header(self):
        return self._header

    @property
    def modified(self):
        return self.header.modified or self.payload.modified

    @property
    def valid(self):
        """ could be True, False, or None (not verified yet) """
        return self._valid

    @property
    def destroy(self):
        return self._destroy

    @destroy.setter
    def destroy(self, value:bool):
        self._destroy = value

    def verify(self, keystore, audience, unverified=None, raise_if_failed=False):
        self._valid = False
        if unverified:
            self.encoded = unverified
        alg = self.header.get('alg', '')
        unverified_kid = self.header.get('kid', '')
        keyitem = keystore.choose_pubkey(kid=unverified_kid)
        pubkey = keyitem if isinstance(keyitem, PyJWK) else PyJWK(jwk_data=keyitem)
        if not pubkey:
            log_args = ['unverified_kid', unverified_kid, 'alg', alg,
                    'msg', 'public key not found on verification',]
            _logger.warning(None, *log_args) # log this because it may be security issue
            return
        try:
            options = {'verify_signature': True, 'verify_exp': True, 'verify_aud': True,}
            verified = jwt.decode(self.encoded, pubkey.key, algorithms=alg,
                        options=options, audience=audience)
            errmsg = 'data inconsistency, self.payload = %s , verified = %s'
            assert self.payload == verified, errmsg % (self.payload, verified)
            self._valid = True
        except Exception as e:
            log_args = ['encoded', self.encoded, 'pubkey', pubkey.key, 'err_msg', ', '.join(e.args)]
            _logger.warning(None, *log_args)
            if raise_if_failed:
                raise
            else:
                verified = None
        return verified


    def encode(self, keystore):
        if self.modified:
            log_args = []
            unverified_kid = self.header.get('kid', '')
            keyitem  = keystore.choose_secret(kid=unverified_kid, randonly=True)
            if isinstance(keyitem, PyJWK):
                secret = keyitem
                keyitem = keyitem._jwk_data
            else:
                secret = PyJWK(jwk_data=keyitem)
            if keyitem.get('kid', None) and unverified_kid != keyitem['kid']:
                log_args.extend(['unverified_kid', unverified_kid, 'verified_kid', keyitem['kid']])
            self.header['kid'] = keyitem.get('kid', unverified_kid)
            # In PyJwt , alg can be `RS256` (for RSA key) or `HS256` (for HMAC key)
            self.header['alg'] = keyitem['alg']
            if secret:
                out = jwt.encode(self.payload, secret.key, algorithm=self.header['alg'],
                        headers=self.header)
            log_args.extend(['alg', keyitem['alg'], 'encode_succeed', any(out),
                'secret_found', secret])
            _logger.debug(None, *log_args)
        else:
            out = self.encoded
        return out

    def default_claims(self, header_kwargs, payld_kwargs):
        self.header.update(header_kwargs, overwrite=False)
        self.payload.update(payld_kwargs, overwrite=False)


class JwkRsaKeygenHandler(RSAKeygenHandler):
    @property
    def algorithm(self):
        if hasattr(self, '_key_size_in_bits'):
            out = 'RS%s' % (self._key_size_in_bits >> 3)
        else:
            out = super().algorithm
        return out

    def generate(self, key_size_in_bits, num_primes=2):
        self._key_size_in_bits = key_size_in_bits
        components = super().generate(key_size_in_bits, num_primes)
        # JWK only recognizes `qi` member as private key component, renaming is required
        components['private']['qi'] = components['private'].pop('qp')
        # each value in components is string that represent very-large number
        # , but JWK requires the key components are encoded with Base64
        def _big_decimal_to_base64(k, comp):
            if comp.get(k, None):
                if isinstance(comp[k] , str):
                    bignum = int(comp[k])
                    comp[k] = to_base64url_uint(bignum).decode('utf-8')
                elif isinstance(comp[k] , list):
                    for item in comp[k]:
                        list(map(partial(_big_decimal_to_base64, comp=item) , item.keys() ))
        list(map(partial(_big_decimal_to_base64, comp=components['private']) , components['private'].keys() ))
        list(map(partial(_big_decimal_to_base64, comp=components['public']) , components['public'].keys() ))
        def _privkey_parser(self, item):
            item.update(components['private'])
        def _pubkey_parser(self, item):
            item.update(components['public'])
        attrs = {'private': _privkey_parser, 'public': _pubkey_parser, 'size': key_size_in_bits,
                'algorithm': self.algorithm, '__slots__':() }
        delattr(self, '_key_size_in_bits')
        return  type("JwkRsaKeyset", (), attrs)()


class RemoteJWKSPersistHandler(AbstractKeystorePersistReadMixin):
    def __init__(self, url, name='default persist handler'):
        self._jwk_client = PyJWKClient(url)
        self._name = name

    def _get_signing_keys(self):
        # TODO, cache response body of jwks
        if not hasattr(self, '_signing_keys'):
            self._signing_keys = self._jwk_client.get_signing_keys()
        return self._signing_keys

    def __len__(self):
        signing_keys = self._get_signing_keys()
        return len(signing_keys)

    def __getitem__(self, key_id):
        signing_keys = self._get_signing_keys()
        signing_key = list(filter(lambda x: x.key_id == key_id, signing_keys))
        if not signing_key:
            raise PyJWKClientError(
                f'Unable to find a signing key that matches: "{key_id}"'
            )
        return signing_key[0]


