import unittest

from check_tls import certificate_error


class CertificateDiagnosticTests(unittest.TestCase):
    def test_localized_schannel_untrusted_root(self):
        self.assertTrue(certificate_error('native-tls: localized message (os error -2146762487)'))
        self.assertTrue(certificate_error('invalid peer certificate: UnknownIssuer'))

    def test_unrelated_network_errors_do_not_pass(self):
        for message in ('connection reset', 'unexpected EOF', 'os error -2146762486', 'panic'):
            self.assertFalse(certificate_error(message))


if __name__ == '__main__':
    unittest.main()
