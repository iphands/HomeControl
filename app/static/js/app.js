
/*global angular, window*/
(function () {
    'use strict';
    var app = angular.module('ColorApp', ['ngResource', 'ngMaterial']),
        priv = {
            events: {
                modeChanged: 'modeChanged'
            }
        };

    app.config(['$resourceProvider', function ($resourceProvider) {
        // Don't strip trailing slashes from calculated URLs
        $resourceProvider.defaults.stripTrailingSlashes = false;
    }]);

    app.controller('ModeCtrl', function ModeController($scope, Mode, $rootScope) {
        $scope.setMode = function () {
            new Mode({mode: $scope.mode}).$save({current: 'current'}, function () {
                $rootScope.$broadcast(priv.events.modeChanged);
            });
        };

        Mode.get({current: 'current'}, function (res) {
            $scope.mode = res.mode;
        });

        Mode.query(function (res) {
            $scope.modes = res;
        });
    });

    priv.sliderCtrl = function (key, multiplier, $scope, Api) {
        multiplier = multiplier || 1;

        $scope.slider = {
            value: 150,
            options: {
                onChange: function () { $scope.set(); },
                floor: 0,
                ceil: 500
            }
        };

        $scope.set = function () {
            var obj = {};
            obj[key] = $scope.slider.value / multiplier;
            new Api(obj).$save({});
        };

        $scope.loadData = function () {
            Api.get(function (res) {
                $scope.slider.value = res[key] * multiplier;
            });
        }

        $scope.$on(priv.events.modeChanged, $scope.loadData);
        $scope.loadData();
    };

    app.controller('DelayCtrl', function ($scope, Delay) {
        priv.sliderCtrl('delay', 1000, $scope, Delay);
    });

    app.controller('BrightnessCtrl', function ($scope, Brightness) {
        priv.sliderCtrl('brightness', 1, $scope, Brightness);
    });

    app.controller('OptsCtrl', function($scope, Opts) {
        $scope.loadData = function () {
            Opts.get(function (res) {
                $scope.opts = res.opts;
            });
        }

        $scope.changeOpt = function (k, v) {
            var obj = {};
            obj[k] = $scope.opts[k];
            new Opts(obj).$save();
        };

        $scope.$on(priv.events.modeChanged, $scope.loadData);
        $scope.loadData();
    });


    app.factory('Mode', function ($resource) {
        return $resource('/modes/:current', { current: '@current' }, {});
    });

    app.factory('Delay', function ($resource) {
        return $resource('/delay', {}, {});
    });

    app.factory('Brightness', function ($resource) {
        return $resource('/brightness', {}, {});
    });

    app.factory('Opts', function ($resource) {
        return $resource('/opts', {}, {});
    });

}());
