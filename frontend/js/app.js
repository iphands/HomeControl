/*global angular, window*/
(function () {
  'use strict';
  var app = angular.module('ColorApp', ['ngResource', 'ngMaterial']),
      priv = {
        events: {
          modeChanged: 'modeChanged',
          stripsLoaded: 'stripsLoaded'
        }
      };

  app.config(['$resourceProvider', function ($resourceProvider) {
    $resourceProvider.defaults.stripTrailingSlashes = false;
  }]);

  // Factories for API resources (defined first so controllers can use them)

  app.factory('Fan', ['$resource', function ($resource) {
    return $resource('/fanbuttons/:op', { op: '@op' }, {});
  }]);

  app.factory('Mode', ['$resource', function ($resource) {
    return $resource('/api/modes/:current', { current: '@current' }, {});
  }]);

  app.factory('Delay', ['$resource', function ($resource) {
    return $resource('/api/delay', {}, {});
  }]);

  app.factory('Brightness', ['$resource', function ($resource) {
    return $resource('/api/brightness', {}, {});
  }]);

  app.factory('Opts', ['$resource', function ($resource) {
    return $resource('/api/opts', {}, {});
  }]);

  app.factory('Strips', ['$resource', function ($resource) {
    return $resource('/api/strips', {}, {});
  }]);

  // Per-strip service factory
  app.factory('StripService', ['$resource', function ($resource) {
    return function (stripId) {
      return {
        Mode: $resource('/api/strips/' + stripId + '/mode', {}, {}),
        Brightness: $resource('/api/strips/' + stripId + '/brightness', {}, {}),
        Delay: $resource('/api/strips/' + stripId + '/delay', {}, {}),
        Opts: $resource('/api/strips/' + stripId + '/opts', {}, {})
      };
    };
  }]);

  // Main controller - loads strips data
  app.controller('MainCtrl', ['$scope', '$rootScope', 'Strips', 'Mode', function ($scope, $rootScope, Strips, Mode) {
    $scope.strips = [];

    function loadStrips() {
      Strips.get(function (res) {
        $scope.strips = res.strips;
        $rootScope.$broadcast(priv.events.stripsLoaded, res.strips);
      });
    }

    // Load modes for child controllers
    Mode.query(function (res) {
      $scope.modes = res;
      $rootScope.modes = res;
    });

    loadStrips();

    // Refresh strips periodically
    $scope.$on(priv.events.modeChanged, function () {
      loadStrips();
    });
  }]);

  // Fan controller
  app.controller('FanCtrl', ['$scope', 'Fan', function ($scope, Fan) {
    $scope.press = function (op) { Fan.get({op: op}); };
  }]);

  // Global controls controller
  app.controller('GlobalCtrl', ['$scope', '$rootScope', 'Mode', 'Brightness', 'Delay', 'Opts', function ($scope, $rootScope, Mode, Brightness, Delay, Opts) {
    $scope.modes = $rootScope.modes || [];
    $scope.mode = '';
    $scope.brightness = 255;
    $scope.delay = 25;
    $scope.opts = {};

    $scope.hasOpts = function () {
      return Object.keys($scope.opts).length > 0;
    };

    $scope.setMode = function () {
      new Mode({mode: $scope.mode}).$save({current: 'current'}, function () {
        $rootScope.$broadcast(priv.events.modeChanged);
        loadOpts();
      });
    };

    $scope.setBrightness = function () {
      new Brightness({brightness: parseInt($scope.brightness)}).$save({});
    };

    $scope.setDelay = function () {
      new Delay({delay: $scope.delay / 1000}).$save({});
    };

    $scope.changeOpt = function (k) {
      var obj = {};
      obj[k] = $scope.opts[k];
      new Opts(obj).$save(function (res) {
        // Update opts from response
      });
    };

    function loadOpts() {
      Opts.get(function (res) {
        $scope.opts = res.opts;
      });
    }

    function loadData() {
      Mode.get({current: 'current'}, function (res) {
        $scope.mode = res.mode;
      });
      Mode.query(function (res) {
        $scope.modes = res;
      });
      Brightness.get(function (res) {
        $scope.brightness = res.brightness;
      });
      Delay.get(function (res) {
        $scope.delay = Math.round(res.delay * 1000);
      });
      loadOpts();
    }

    loadData();
  }]);

  // Per-strip controller
  app.controller('StripCtrl', ['$scope', '$rootScope', 'StripService', function ($scope, $rootScope, StripService) {
    var stripId = $scope.strip.id;
    var service = StripService(stripId);

    $scope.modes = $rootScope.modes || [];
    $scope.stripMode = $scope.strip.mode;
    $scope.stripBrightness = $scope.strip.brightness;
    $scope.stripDelay = Math.round($scope.strip.delay * 1000);
    $scope.stripOpts = {};
    $scope.isCollapsed = false;

    $scope.hasOpts = function () {
      return Object.keys($scope.stripOpts).length > 0;
    };

    $scope.toggleCollapse = function () {
      $scope.isCollapsed = !$scope.isCollapsed;
    };

    $scope.setMode = function () {
      service.Mode.save({mode: $scope.stripMode}, function () {
        loadOpts();
      });
    };

    $scope.setBrightness = function () {
      service.Brightness.save({brightness: parseInt($scope.stripBrightness)});
    };

    $scope.setDelay = function () {
      service.Delay.save({delay: $scope.stripDelay / 1000});
    };

    $scope.changeOpt = function (k) {
      var obj = {};
      obj[k] = $scope.stripOpts[k];
      service.Opts.save(obj);
    };

    function loadOpts() {
      service.Opts.get(function (res) {
        $scope.stripOpts = res.opts;
      });
    }

    function loadData() {
      service.Mode.get(function (res) {
        $scope.stripMode = res.mode;
      });
      service.Brightness.get(function (res) {
        $scope.stripBrightness = res.brightness;
      });
      service.Delay.get(function (res) {
        $scope.stripDelay = Math.round(res.delay * 1000);
      });
      loadOpts();
    }

    // Update modes list when available
    $scope.$watch(function () { return $rootScope.modes; }, function (newVal) {
      if (newVal) {
        $scope.modes = newVal;
      }
    });

    // Load initial data
    loadData();

    // Refresh on global mode change
    $scope.$on(priv.events.modeChanged, function () {
      loadData();
    });
  }]);

}());
